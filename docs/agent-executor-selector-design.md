# AQBot Agent Executor Selector Design

## 1. Purpose

This document designs the Agent-mode executor selector in the chat input area.

The goal is to let users choose which agent executes an Agent-mode request
without turning chat into a task console.

Target UI:

```text
[Ask] [Agent v]

Agent v:
  AQBot Local Agent
  OpenCode
  Claude Code
  Codex
  OpenClaw Remote
  Manage agents...
```

## 2. Product Positioning

AQBot remains local-agent-first.

Executor selector rules:

- Ask mode uses normal chat provider flow.
- Agent mode defaults to `AQBot Local Agent`.
- External/code executors are optional plugins.
- The selector chooses an execution backend, not a model provider.
- Detailed configuration belongs in Settings, not the input bar.

## 3. User Experience

### 3.1 Input Bar

Current:

```text
[Ask / Agent]
```

Target:

```text
[Ask] [Agent: AQBot Local v] [workspace] [permission]
```

When in Ask mode:

```text
[Ask] [Agent v]
```

When switching to Agent mode, AQBot selects:

1. Last selected executor for this conversation.
2. User default executor.
3. `aqbot-local`.

### 3.2 Executor Menu

Menu groups:

```text
Local
  AQBot Local Agent

Code Agents
  OpenCode
  Claude Code
  Codex

Remote
  OpenClaw Remote
  Other configured external agents

Settings
  Manage agents...
```

Each item shows:

- Name.
- Type icon.
- Availability state.
- Short description.

Unavailable executor examples:

- `Claude Code` not installed.
- `OpenCode` not configured.
- `OpenClaw Remote` disabled.

Unavailable items should be visible but disabled, with a setup hint.

### 3.3 Status Bar

Agent mode status line should show:

```text
Agent 路 OpenCode
Workspace 路 D:/project/example
Permission 路 Default
```

Keep it compact. Long paths should truncate.

### 3.4 Main Chat

Main chat should show:

- A small progress surface while agent is working.
- Final result as a normal assistant message.
- Clear human-readable failure messages.

Main chat should not show:

- Raw task ids.
- Raw JSON.
- Persistent task banners.
- Sync/Retry buttons unless a diagnostics drawer is opened.

## 4. Executor Types

```ts
type AgentExecutorKind =
  | 'aqbot_local'
  | 'code_cli'
  | 'external_connector'
  | 'custom';
```

Built-in executor ids:

```text
aqbot-local
opencode
claude-code
codex
```

External executor ids should reference `external_agents.id`:

```text
external:<external_agent_id>
```

## 5. Executor Registry

AQBot should maintain a normalized executor registry.

```ts
interface AgentExecutor {
  id: string;
  name: string;
  kind: AgentExecutorKind;
  enabled: boolean;
  available: boolean;
  description?: string;
  icon?: string;
  source: 'builtin' | 'external';
  externalAgentId?: string;
  capabilities: {
    code?: boolean;
    files?: boolean;
    remote?: boolean;
    longRunning?: boolean;
    tools?: boolean;
  };
  diagnostics?: {
    status?: string;
    setupHint?: string;
  };
}
```

Registry sources:

- Built-in local AQBot Agent.
- Built-in code CLI adapters.
- External agent records from `external_agents`.

## 6. Persistence

### 6.1 Conversation-Level Selection

Store selected executor per conversation.

Preferred future field:

```text
conversations.agent_executor_id TEXT NULL
```

Near-term non-migration option:

```text
agent_sessions.executor_id TEXT NULL
```

Fallback:

- `localStorage["aqbot:agent-executor:<conversationId>"]`

### 6.2 User Default

Later setting:

```text
settings.default_agent_executor_id
```

Default:

```text
aqbot-local
```

## 7. Dispatch Routing

When user sends in Agent mode:

```text
if executor == aqbot-local:
  sendAgentMessage(...)

if executor.kind == code_cli:
  dispatch code-agent run

if executor.kind == external_connector:
  dispatch external-agent task
```

### 7.1 Local AQBot Agent

Uses existing:

```ts
sendAgentMessage(text, attachments)
```

### 7.2 Code CLI Agent

Future adapters:

- OpenCode adapter.
- Claude Code adapter.
- Codex adapter.

They should use the same task/result standard from
`docs/agent-connector-standard.md`, but may execute locally through a CLI rather
than HTTP.

### 7.3 External Connector

Uses existing external-agent connector layer:

```ts
dispatch_external_agent_task
```

But chat UI should hide raw task mechanics.

## 8. Code Agent Adapter Boundary

Code agents are specialized executors.

They need:

- Workspace directory.
- Permission mode.
- Model/provider config if required by the CLI.
- Environment variables.
- Output parser.

Adapter responsibilities:

- Check installation.
- Build command safely.
- Stream progress.
- Capture stdout/stderr.
- Detect changed files.
- Return normalized result.

Adapter must not:

- Run without a workspace.
- Hide file edits from AQBot.
- Bypass permission mode.
- Write secrets into logs.

## 9. Suggested UI Phases

### Phase 1: Design-Only / Low-Risk

- Keep current Agent mode behavior.
- Add docs for executor selector.
- Hide external-agent controls from main chat.
- Keep External Agents in Settings.

### Phase 2: Selector Shell

- Add selector menu in Agent mode.
- Only `AQBot Local Agent` is active.
- Show disabled placeholders for OpenCode, Claude Code, Codex.
- Include `OpenClaw Remote` only if an external agent exists and is enabled.
- Persist selection in localStorage.

### Phase 3: External Connector Routing

- Allow selecting `external:<id>`.
- Dispatch through existing external-agent layer.
- Insert results as normal assistant messages.
- Move raw task controls into diagnostics.

### Phase 4: Code CLI Executors

- Add installation checks.
- Implement one executor first, likely OpenCode or Codex.
- Add permission-aware local command runner.
- Add result parser and changed-file summary.

### Phase 5: Polishing

- Add default executor setting.
- Add per-project executor preference.
- Add routing rules.
- Add diagnostics drawer.

## 10. First Implementation Recommendation

Do not start by integrating every external tool.

Start with:

1. `AQBot Local Agent` as the only active executor.
2. Selector UI showing future executor slots.
3. `OpenClaw Remote` listed only in diagnostics/settings.
4. Then add one code executor adapter.

Recommended first code executor:

- If the goal is local project automation: `OpenCode`.
- If the goal is Claude-specific code workflow: `Claude Code`.
- If the goal is aligning with AQBot's current development environment:
  `Codex`.

## 11. Open Questions

- Should code executors be allowed to modify files by default, or only after
  explicit permission?
- Should external remote agents ever be selectable from the main input bar, or
  only from a secondary "More executors" menu?
- Should executor selection be per conversation, per workspace, or global?
- How should AQBot summarize file changes from third-party code agents?

