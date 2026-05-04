# External Agent Mock Quickstart

This local mock service is the fastest way to validate AQBot's external-agent flow before switching to a real OpenClaw-compatible backend.

## Start the mock service

From the repo root:

```bash
pnpm external-agent:mock
```

Default address:

- Base URL: `http://127.0.0.1:8788`
- Health check: `http://127.0.0.1:8788/health`

Optional environment variables:

- `AQBOT_EXTERNAL_AGENT_MOCK_PORT`
- `AQBOT_EXTERNAL_AGENT_MOCK_HOST`
- `AQBOT_EXTERNAL_AGENT_MOCK_DELAY_MS`

## Configure AQBot

In `Settings -> External Agents`, create one of the following:

### Option A: Generic HTTP

- Name: `Local Mock Generic`
- Kind: `generic_http`
- Base URL: `http://127.0.0.1:8788`
- Auth: `none`

Suggested capabilities JSON:

```json
{
  "taskKinds": ["general"],
  "resultIngest": ["assistant_message", "artifacts"],
  "notes": "Local mock service for AQBot integration testing"
}
```

### Option B: OpenClaw Compatible

- Name: `Local Mock OpenClaw`
- Kind: `openclaw`
- Base URL: `http://127.0.0.1:8788`
- Auth: `none`

You can keep the same capabilities JSON.

## Test flow

1. Click `Test` in the external-agent settings page.
2. Open a conversation in AQBot.
3. Use the `External Agent` button in the chat header.
4. Submit a short task.
5. Wait a few seconds, or click `Sync`.

Expected result:

- A task is created in AQBot.
- Task status moves from `running` to `completed`.
- A result message is written back into the conversation.
- A markdown artifact is attached and saved through AQBot's file pipeline.

## Mock-specific context options

You can override a couple of behaviors by including these values inside the task context:

```json
{
  "mock": {
    "delayMs": 1500,
    "includeArtifact": true
  }
}
```

## Switching from mock to a real OpenClaw service

Once the AQBot flow is validated, keep the same AQBot usage path and change only:

- `Kind`: `openclaw`
- `Base URL`: your real service URL
- `Auth`: set according to the real backend

That lets us validate AQBot first, then replace the mock transport with the real one without changing the chat-side workflow.
