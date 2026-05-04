#!/usr/bin/env python3
"""
Development-only OpenAI-compatible proxy backed by AQBot's local provider config.

This is intentionally small and operational: it lets external tools on the LAN
call the models already configured in AQBot without copying provider API keys to
another machine. It reads %USERPROFILE%/.aqbot/aqbot.db and master.key at request
time, decrypts the selected provider key in memory, forwards the request, and
never logs secrets.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import sqlite3
import sys
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

from cryptography.hazmat.primitives.ciphers.aead import AESGCM


NONCE_SIZE = 12


def aqbot_home() -> Path:
    return Path(os.environ.get("AQBOT_HOME") or Path.home() / ".aqbot")


def resolve_base_url(api_host: str) -> str:
    trimmed = api_host.rstrip("/")
    last = trimmed.rsplit("/", 1)[-1]
    if len(last) >= 2 and last[0] == "v" and last[1].isdigit():
        return trimmed
    return f"{trimmed}/v1"


def decrypt_key(encrypted: str, master_key: bytes) -> str:
    combined = base64.b64decode(encrypted)
    if len(combined) < NONCE_SIZE:
        raise ValueError("invalid encrypted key")
    nonce = combined[:NONCE_SIZE]
    ciphertext = combined[NONCE_SIZE:]
    return AESGCM(master_key).decrypt(nonce, ciphertext, None).decode("utf-8")


def load_model(model_id: str) -> dict[str, Any]:
    db_path = aqbot_home() / "aqbot.db"
    master_key_path = aqbot_home() / "master.key"
    master_key = master_key_path.read_bytes()
    if len(master_key) != 32:
        raise RuntimeError("AQBot master.key must be 32 bytes")

    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    try:
        row = conn.execute(
            """
            select
                p.id as provider_id,
                p.name as provider_name,
                p.api_host,
                p.api_path,
                m.model_id,
                k.key_encrypted
            from models m
            join providers p on p.id = m.provider_id
            join provider_keys k on k.provider_id = p.id
            where p.enabled = 1
              and m.enabled = 1
              and k.enabled = 1
              and m.model_id = ?
            order by k.rotation_index asc, k.created_at asc
            limit 1
            """,
            (model_id,),
        ).fetchone()
    finally:
        conn.close()

    if row is None:
        raise KeyError(f"model is not enabled in AQBot: {model_id}")

    api_key = decrypt_key(row["key_encrypted"], master_key)
    base = resolve_base_url(row["api_host"])
    api_path = (row["api_path"] or "/chat/completions").strip()
    if api_path.endswith("!"):
        api_path = api_path[:-1]
    if not api_path.startswith("/"):
        api_path = f"/{api_path}"
    if api_path.startswith("/v") and len(api_path) > 2 and api_path[2].isdigit():
        parts = api_path.split("/", 2)
        api_path = f"/{parts[2]}" if len(parts) > 2 else ""

    return {
        "provider_name": row["provider_name"],
        "model_id": row["model_id"],
        "url": f"{base}{api_path}",
        "api_key": api_key,
    }


def list_models() -> dict[str, Any]:
    conn = sqlite3.connect(aqbot_home() / "aqbot.db")
    conn.row_factory = sqlite3.Row
    try:
        rows = conn.execute(
            """
            select m.model_id, p.name as provider_name, p.created_at
            from models m
            join providers p on p.id = m.provider_id
            where p.enabled = 1
              and m.enabled = 1
              and coalesce(m.model_type, 'chat') = 'chat'
            order by m.model_id asc
            """
        ).fetchall()
    finally:
        conn.close()
    return {
        "object": "list",
        "data": [
            {
                "id": row["model_id"],
                "object": "model",
                "created": row["created_at"],
                "owned_by": row["provider_name"],
            }
            for row in rows
        ],
    }


def expected_token() -> str:
    return os.environ.get("AQBOT_DEV_PROXY_TOKEN", "aqbot-local")


class Handler(BaseHTTPRequestHandler):
    server_version = "AQBotDevProxy/0.1"

    def log_message(self, fmt: str, *args: Any) -> None:
        sys.stderr.write("%s - %s\n" % (self.address_string(), fmt % args))

    def send_json(self, status: int, payload: dict[str, Any]) -> None:
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("content-type", "application/json; charset=utf-8")
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def require_auth(self) -> bool:
        header = self.headers.get("authorization") or ""
        expected = f"Bearer {expected_token()}"
        if header.strip() == expected:
            return True
        self.send_json(401, {"error": {"message": "unauthorized"}})
        return False

    def do_GET(self) -> None:
        if self.path == "/health":
            self.send_json(200, {"status": "ok", "service": "aqbot-dev-openai-proxy"})
            return
        if self.path == "/v1/models":
            if not self.require_auth():
                return
            try:
                self.send_json(200, list_models())
            except Exception as exc:
                self.send_json(500, {"error": {"message": str(exc)}})
            return
        self.send_json(404, {"error": {"message": "not found"}})

    def do_POST(self) -> None:
        if self.path != "/v1/chat/completions":
            self.send_json(404, {"error": {"message": "not found"}})
            return
        if not self.require_auth():
            return

        try:
            length = int(self.headers.get("content-length") or "0")
            body = self.rfile.read(length)
            payload = json.loads(body.decode("utf-8"))
            model = str(payload.get("model") or "").strip()
            selected = load_model(model)
        except Exception as exc:
            self.send_json(400, {"error": {"message": str(exc)}})
            return

        request = urllib.request.Request(
            selected["url"],
            data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
            headers={
                "authorization": f"Bearer {selected['api_key']}",
                "content-type": "application/json",
            },
            method="POST",
        )

        try:
            with urllib.request.urlopen(request, timeout=180) as upstream:
                self.send_response(upstream.status)
                content_type = upstream.headers.get("content-type", "application/json")
                self.send_header("content-type", content_type)
                self.end_headers()
                while True:
                    chunk = upstream.read(8192)
                    if not chunk:
                        break
                    self.wfile.write(chunk)
                    self.wfile.flush()
        except urllib.error.HTTPError as exc:
            data = exc.read()
            self.send_response(exc.code)
            self.send_header("content-type", exc.headers.get("content-type", "application/json"))
            self.send_header("content-length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        except Exception as exc:
            self.send_json(502, {"error": {"message": str(exc)}})


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="0.0.0.0")
    parser.add_argument("--port", type=int, default=18080)
    args = parser.parse_args()

    server = ThreadingHTTPServer((args.host, args.port), Handler)
    print(f"AQBot dev OpenAI proxy listening on http://{args.host}:{args.port}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
