# AQBot Agent Connector Standard

## 1. Purpose

This document defines the standard contract for connecting built-in and external
agents to AQBot.

The goal is to make agents pluggable without making the user understand each
agent runtime's internal task model. AQBot should provide one coherent product
experience:

- Chat remains the primary user interface.
- Agent mode becomes a deliberate execution mode.
- External agents behave like capability providers, not separate apps embedded
  into the chat surface.
- Connector details stay behind a stable AQBot contract.

## 2. Terminology

| Term | Meaning |
| --- | --- |
| Agent | A worker that can execute a task, either inside AQBot or outside AQBot. |
| Connector | AQBot-side adapter that talks to one agent runtime or service. |
| External agent | An agent hosted outside AQBot, such as OpenClaw, NanoClaw, Dify, LangGraph, or a custom HTTP worker. |
| Task | A normalized unit of work sent from AQBot to an agent. |
| Run | One execution attempt for a task. A retry creates a new run. |
| Result | Final or partial output returned by an agent. |
| Artifact | File, URL, code patch, generated media, or other durable output from a task. |
| Memory candidate | Information returned by an agent that AQBot may offer to save into memory. |

## 3. Product Model

AQBot should expose three user-facing layers.

### 3.1 Ask Mode

Ask mode is the normal chat assistant.

Use it for:

- Short answers.
- Writing and summarization.
- Knowledge-base Q&A.
- Memory-assisted conversation.
- Lightweight analysis.

Ask mode should not expose task internals. If it uses an agent behind the scenes,
the final response should still appear as a normal assistant message.

### 3.2 Agent Mode

Agent mode is for intentional execution.

Use it for:

- Multi-step work.
- Code or file operations.
- Long-running jobs.
- Remote execution.
- Research workflows.
- Automation setup.

Agent mode may show progress, but only as user-meaningful state:

- `Waiting`
- `Working`
- `Needs input`
- `Completed`
- `Failed`

It should not show transport details like `/tasks/:id`, `Sync`, raw payloads, or
connector-specific JSON unless the user opens diagnostics.

### 3.3 Diagnostics Mode

Diagnostics mode is for developers and power users.

It may show:

- Connector health.
- Raw task ids.
- External task ids.
- Events.
- Request and response payloads.
- Retry and sync controls.

Diagnostics must be opt-in and should not occupy the main chat content area.

## 4. Capability Model

Each agent declares capabilities in a stable JSON shape.

```json
{
  "version": 1,
  "taskKinds": ["chat", "code", "research", "automation"],
  "modalities": {
    "input": ["text", "image", "file"],
    "output": ["text", "file", "artifact"]
  },
  "execution": {
    "supportsStreaming": false,
    "supportsCancel": false,
    "supportsResume": false,
    "supportsLongRunning": true
  },
  "resultIngest": ["assistant_message", "artifacts", "memory_candidates"],
  "limits": {
    "maxInputChars": 60000,
    "maxFiles": 20
  },
  "vendor": {
    "kind": "openclaw",
    "agentId": "main"
  }
}
```

Rules:

- `version` is required.
- `taskKinds` is required.
- `vendor` is optional and connector-specific.
- AQBot must ignore unknown fields.
- Connectors must not require UI code to know vendor-specific fields.

## 5. Task Contract

AQBot dispatches normalized tasks to connectors.

```json
{
  "id": "aqtask_...",
  "conversationId": "conv_...",
  "sourceMessageId": "msg_...",
  "kind": "research",
  "title": "Research request",
  "input": {
    "text": "Find and summarize...",
    "attachments": [
      {
        "id": "file_...",
        "name": "brief.pdf",
        "mimeType": "application/pdf",
        "uri": "aqbot://documents/files/brief.pdf"
      }
    ]
  },
  "context": {
    "conversation": {
      "title": "Project planning",
      "recentMessages": []
    },
    "knowledge": {
      "snippets": []
    },
    "memory": {
      "snippets": []
    },
    "user": {
      "timezone": "Asia/Shanghai"
    }
  },
  "policy": {
    "permissionMode": "ask",
    "network": "allowed",
    "filesystem": "documents_only"
  },
  "createdAt": 1777890000
}
```

Rules:

- AQBot task ids are always AQBot-owned.
- `conversationId` and `sourceMessageId` may be null for detached jobs.
- Paths passed to external agents must use AQBot-safe URIs or user-visible
  document paths. Do not expose hidden AQBot config paths unless diagnostics is
  explicitly enabled.
- Attachments stored in the database must follow the storage policy: relative
  paths under the documents root for user files.

## 6. Connector HTTP Contract

For HTTP-based connectors, AQBot v1 standardizes the following endpoints.

### 6.1 Health

```http
GET /health
```

Response:

```json
{
  "ok": true,
  "service": "openclaw-bridge",
  "version": "1.0.0",
  "capabilities": {}
}
```

### 6.2 Dispatch Task

```http
POST /tasks
```

Request:

```json
{
  "task": {},
  "agent": {
    "id": "agent_...",
    "kind": "openclaw",
    "capabilitiesJson": "{}"
  }
}
```

Response:

```json
{
  "externalTaskId": "remote-task-id",
  "status": "running",
  "summary": "Task accepted"
}
```

For synchronous agents, the response may be final:

```json
{
  "externalTaskId": "remote-task-id",
  "status": "completed",
  "result": {
    "content": "Final answer"
  }
}
```

### 6.3 Fetch Task

```http
GET /tasks/{externalTaskId}
```

Response:

```json
{
  "externalTaskId": "remote-task-id",
  "status": "completed",
  "summary": "Done",
  "result": {}
}
```

### 6.4 Fetch Result

```http
GET /results/{externalTaskId}
```

Response:

```json
{
  "content": "Final answer",
  "artifacts": [],
  "memoryCandidates": []
}
```

### 6.5 Cancel Task

Optional in v1:

```http
POST /tasks/{externalTaskId}/cancel
```

## 7. Status Contract

AQBot supports these normalized statuses:

| Status | Meaning | UI treatment |
| --- | --- | --- |
| `queued` | Accepted but not started | Subtle progress |
| `running` | Work is in progress | Progress indicator |
| `needs_input` | Agent needs user confirmation or data | Ask user card |
| `completed` | Final result is available | Ingest result |
| `failed` | Task ended with an error | Human-readable error |
| `cancelled` | User or system cancelled the task | Neutral terminal state |

Connector-specific statuses must map into this set.

## 8. Result Contract

Agent results should be normalized before they reach chat UI.

```json
{
  "content": "Human-readable final answer.",
  "format": "markdown",
  "artifacts": [
    {
      "type": "file",
      "name": "report.md",
      "uri": "aqbot://documents/files/report.md",
      "mimeType": "text/markdown"
    }
  ],
  "memoryCandidates": [
    {
      "text": "The user prefers concise Chinese status updates.",
      "scope": "user_preference",
      "confidence": 0.8
    }
  ],
  "diagnostics": {
    "provider": "openclaw",
    "model": "MiniMax-M2.7",
    "durationMs": 42000
  }
}
```

Rules:

- `content` is what can become a normal assistant message.
- `artifacts` are stored or linked through AQBot's file center.
- `memoryCandidates` are suggestions, not automatic permanent memory unless the
  user has enabled such policy.
- `diagnostics` is hidden by default.

## 9. Authentication Contract

AQBot v1 supports:

```json
{ "type": "none" }
{ "type": "bearer", "token": "..." }
{ "type": "api_key", "header": "X-API-Key", "apiKey": "..." }
```

Compatibility rule:

- AQBot must accept both `apiKey` and legacy `api_key`.
- UI should write `apiKey`.
- Connector internals should never log secrets.

Transport rule:

- Local and LAN connector calls should not be routed through system HTTP proxy by
  default.
- Proxy support can be added later as an explicit connector setting.

## 10. UI Standard

### 10.1 Main Chat

Main chat may show:

- A small inline progress message.
- Final assistant result.
- A concise failure message.

Main chat must not show by default:

- Raw task ids.
- Raw JSON.
- Persistent task banners.
- Sync/Retry controls occupying the message viewport.

### 10.2 Header Agent Controls

Header may show:

- Agent mode toggle or selected agent.
- Running task count badge.
- Diagnostics popover entry.

### 10.3 Diagnostics

Diagnostics can include:

- Task list.
- Event list.
- Sync.
- Retry.
- Raw payloads.
- Connector health.

Diagnostics belongs in:

- Agent settings page.
- Task detail modal.
- Developer/debug drawer.

## 11. Connector Implementation Boundary

Each connector owns:

- Endpoint mapping.
- Auth header mapping.
- Vendor payload conversion.
- Status mapping.
- Result extraction.
- Error normalization.

Core chat owns:

- Conversation state.
- Message rendering.
- User-facing task progress.
- Result insertion.
- Memory/knowledge/file ingest decisions.

Connectors must not directly write AQBot messages or memory. They return
normalized results; AQBot performs ingestion.

## 12. Initial Built-In Connectors

### 12.1 Generic HTTP

Purpose:

- Simple local or remote workers.
- Internal prototypes.
- Services that already speak AQBot's standard `/tasks` contract.

### 12.2 OpenClaw Compatible

Purpose:

- OpenClaw gateway or compatibility bridge.
- Remote execution on another machine.
- Multi-tool agent execution.

Vendor capability example:

```json
{
  "version": 1,
  "taskKinds": ["chat", "code", "research", "automation"],
  "resultIngest": ["assistant_message", "artifacts"],
  "vendor": {
    "kind": "openclaw",
    "agentId": "main"
  }
}
```

### 12.3 NanoClaw Compatible

Purpose:

- NanoClaw-like local services.
- Lightweight task execution.

## 13. Future Connectors

Candidates:

- Dify workflow or agent app.
- Coze bot.
- LangGraph server.
- CrewAI service.
- AutoGen service.
- MCP worker.
- Local script worker.
- Container worker.
- Company-specific internal agent.

New connector checklist:

- Define capability JSON.
- Implement dispatch/status/result mapping.
- Map errors into AQBot normalized errors.
- Add health test.
- Add fixture/mock test.
- Document setup.

## 14. Versioning

The standard is versioned with an integer `version` field in capabilities.

Rules:

- v1 clients must ignore unknown fields.
- Breaking changes require v2.
- Connectors can expose their own vendor version in `vendor.version`.

## 15. V1 Implementation Plan

### Phase 1: Normalize Current Implementation

- Keep existing `external_agents` and `agent_tasks` tables.
- Rename UI copy from "External Tasks" to user-facing "Agent runs" only in
  diagnostics.
- Hide task banners from the main message viewport.
- Ensure API key auth accepts `apiKey` and legacy `api_key`.
- Ensure LAN connector requests bypass system proxy by default.

### Phase 2: Productize Agent Mode

- Add a clear Ask/Agent mode distinction in chat.
- In Agent mode, show a lightweight progress message.
- Auto-sync running tasks.
- Insert final results as normal assistant messages.
- Move Sync/Retry into a task detail popover or diagnostics drawer.

### Phase 3: Connector SDK

- Extract connector trait and common HTTP helpers.
- Add connector fixtures.
- Add a mock connector test server.
- Document how to add a new connector.

### Phase 4: Result Ingest

- Standardize artifacts.
- Add memory candidate review.
- Add file-center integration.
- Add knowledge-base import action.

## 16. Non-Goals For V1

- Deeply merging OpenClaw internals into AQBot.
- Making every external agent share AQBot memory automatically.
- Replacing AQBot's built-in chat provider pipeline.
- Showing raw external task state in normal chat by default.
- Supporting every third-party agent framework before the connector standard is stable.

