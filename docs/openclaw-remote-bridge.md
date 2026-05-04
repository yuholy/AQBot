# OpenClaw Remote Bridge

AQBot can connect to an OpenClaw gateway through the OpenClaw-compatible external-agent connector.

## Current Test Host

- Host: `192.168.10.25`
- OpenClaw gateway: `http://192.168.10.25:18789`
- AQBot compatibility bridge: `http://192.168.10.25:18792`
- AQBot dev OpenAI proxy: `http://192.168.10.200:18080/v1`
- Bridge service: `aqbot-openclaw-bridge.service`
- Bridge script on remote host: `/root/.aqbot-openclaw-bridge/bridge.mjs`

The bridge exposes AQBot's expected HTTP contract:

- `GET /health`
- `POST /tasks`
- `GET /tasks/:id`
- `GET /results/:id`

Internally, it calls:

```bash
/usr/bin/openclaw agent --agent main --message "<prompt>" --json
```

## AQBot Settings

Create an external agent:

- Kind: `openclaw`
- Base URL: `http://192.168.10.25:18792`
- Auth: `api_key`
- API key header: `X-API-Key`

Suggested capabilities JSON:

```json
{
  "openclawAgentId": "main",
  "taskKinds": ["general"],
  "resultIngest": ["assistant_message", "artifacts"]
}
```

Do not commit the bridge API key into the repository. Keep it only in AQBot settings or a local secret store.

## Current Model Route

The remote OpenClaw host is configured with a custom OpenAI-compatible provider:

- Provider id: `aqbot`
- Base URL: `http://192.168.10.200:18080/v1`
- API: `openai-completions`
- Default model: `aqbot/deepseek-v4-flash`
- Private-network access: enabled for this provider

The local dev proxy is started from this repository:

```bash
python scripts/aqbot-dev-openai-proxy.py --host 0.0.0.0 --port 18080
```

It reads AQBot's local `%USERPROFILE%\.aqbot\aqbot.db` and `master.key`, decrypts the selected provider key in memory, and forwards requests to the provider already configured in AQBot. It requires:

```http
Authorization: Bearer aqbot-local
```

This is a development bridge for integration testing. For production, prefer AQBot's built-in Gateway once it can be started as a LAN-reachable service with an explicit gateway key.

## Verification Notes

The bridge, gateway, and AQBot-backed model route were reachable during integration testing.

Observed remote state:

- OpenClaw gateway was restarted on `18789`.
- Bridge was installed and started on `18792`.
- AQBot-style task dispatch returned a valid `externalTaskId`.
- Result polling returned a terminal `completed` status from OpenClaw.
- OpenClaw execution metadata showed provider `aqbot` and model `deepseek-v4-flash`.

Earlier testing against the remote MiniMax provider failed because the remote host's configured MiniMax key was rejected by the provider:

```text
HTTP 401 authentication_error: invalid api key
```

That failure is bypassed by routing OpenClaw through AQBot's local provider configuration.
