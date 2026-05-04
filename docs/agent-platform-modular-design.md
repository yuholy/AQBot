# AQBot Modular Agent Platform Design

## 1. Purpose

This document defines a modular design for evolving AQBot into a personal AI assistant platform while keeping the existing codebase stable and minimizing changes to upstream submodules.

For the concrete connector protocol, task shape, status contract, result shape,
and UI rules, see [Agent Connector Standard](./agent-connector-standard.md).

The design goal is not to merge external agent runtimes deeply into AQBot. Instead, AQBot should become the user's control plane:

- A unified desktop entrypoint.
- A long-term memory center.
- A knowledge-base center.
- A task and result console.
- A router for local and external agent execution.

External OpenClaw-like services, NanoClaw-like services, custom workers, MCP servers, and automation services should remain pluggable execution providers.

## 2. Design Principles

### 2.1 Keep AQBot as the System of Record

AQBot should own the durable user-facing state:

- Conversations.
- Messages.
- Knowledge bases.
- Memory entries.
- Skills.
- External agent configuration.
- Task records.
- Result summaries.

External agents may keep their own runtime state, but they should not become the primary source of truth for the user's long-term assistant memory.

### 2.2 Prefer Additive Modules Over Core Rewrites

The first implementation should add new modules around the existing system rather than replacing current chat, provider, memory, or agent internals.

Preferred pattern:

```text
Existing AQBot feature
  + small integration point
  + new modular bridge
  + external connector
```

Avoid:

```text
Rewrite existing chat/agent runtime around one external framework
```

### 2.3 External Agents Are Execution Plane

External agents should be treated as workers or execution services. They can:

- Run long tasks.
- Connect to external channels.
- Execute in containers.
- Handle scheduled jobs.
- Call specialized tools.
- Return results, logs, artifacts, and candidate memories.

They should not directly own AQBot's user memory, knowledge base, or main conversation history.

### 2.4 Use Stable Contracts

AQBot should define its own stable contracts for:

- Registered external agents.
- Dispatchable tasks.
- Task events.
- Returned artifacts.
- Candidate memories.

Each external service should adapt to those contracts through a connector.

## 3. Target Architecture

```text
User
  |
  v
AQBot UI
  |
  v
AQBot Core
  |-- Conversations
  |-- Messages
  |-- Knowledge
  |-- Memory
  |-- Skills
  |-- Local Agent Mode
  |-- Gateway
  |
  v
External Agent Layer
  |-- Agent Registry
  |-- Task Bridge
  |-- Connector Registry
  |-- Result Ingest
  |-- Memory Candidate Ingest
  |
  v
External Execution Providers
  |-- NanoClaw-like service
  |-- OpenClaw-like service
  |-- Custom HTTP worker
  |-- MCP-backed worker
  |-- Containerized automation worker
```

AQBot remains the user's main workspace. External providers perform work and report back.

## 4. Module Design

### 4.1 External Agent Registry

The registry stores which external services AQBot can call.

Responsibilities:

- Create, update, enable, disable external agents.
- Store connection information.
- Store declared capabilities.
- Store authentication metadata.
- Provide a typed list of available external execution targets.

Suggested backend module:

```text
src-tauri/src/external_agents/
  mod.rs
  registry.rs
  types.rs
```

Suggested database table:

```text
external_agents
  id TEXT PRIMARY KEY
  name TEXT NOT NULL
  kind TEXT NOT NULL
  base_url TEXT
  auth_type TEXT
  auth_config_json TEXT
  capabilities_json TEXT NOT NULL
  enabled INTEGER NOT NULL
  created_at INTEGER NOT NULL
  updated_at INTEGER NOT NULL
```

Example `kind` values:

- `custom_http`
- `nanoclaw`
- `openclaw`
- `mcp_worker`
- `local_worker`

### 4.2 Task Bridge

The task bridge converts an AQBot intent into a normalized external task.

Responsibilities:

- Build task input from a conversation, user prompt, selected files, knowledge snippets, and memory snippets.
- Select a target external agent.
- Create a durable AQBot task record before dispatch.
- Pass the normalized task to a connector.

Suggested backend module:

```text
src-tauri/src/external_agents/
  task_bridge.rs
  task_router.rs
```

Normalized task shape:

```rust
pub struct ExternalAgentTask {
    pub id: String,
    pub conversation_id: Option<String>,
    pub source_message_id: Option<String>,
    pub title: String,
    pub kind: ExternalTaskKind,
    pub input: ExternalTaskInput,
    pub context: ExternalTaskContext,
    pub created_at: i64,
}
```

Task kinds:

- `chat`
- `code_analysis`
- `document_work`
- `automation`
- `channel_message`
- `tool_run`
- `long_running_job`

### 4.3 Connector Registry

The connector registry maps an external agent kind to an implementation.

Responsibilities:

- Hide provider-specific APIs behind a small common trait.
- Keep NanoClaw/OpenClaw/custom HTTP details outside core chat logic.
- Allow new external services to be added without changing existing agent code.

Suggested trait:

```rust
#[async_trait::async_trait]
pub trait ExternalAgentConnector: Send + Sync {
    fn kind(&self) -> &'static str;

    async fn dispatch_task(
        &self,
        agent: &ExternalAgentConfig,
        task: &ExternalAgentTask,
    ) -> Result<ExternalDispatchResult>;

    async fn fetch_task_status(
        &self,
        agent: &ExternalAgentConfig,
        external_task_id: &str,
    ) -> Result<ExternalTaskStatus>;

    async fn fetch_task_result(
        &self,
        agent: &ExternalAgentConfig,
        external_task_id: &str,
    ) -> Result<Option<ExternalTaskResult>>;
}
```

Initial connectors:

- `custom_http_connector`: the first generic connector.
- `nanoclaw_connector`: adapter for NanoClaw-like APIs.
- `openclaw_connector`: adapter for OpenClaw-like APIs when needed.

### 4.4 Task Records

AQBot should persist every external task before dispatch and update it as status changes.

Suggested table:

```text
agent_tasks
  id TEXT PRIMARY KEY
  conversation_id TEXT
  source_message_id TEXT
  external_agent_id TEXT
  external_task_id TEXT
  kind TEXT NOT NULL
  status TEXT NOT NULL
  title TEXT NOT NULL
  request_payload_json TEXT NOT NULL
  result_payload_json TEXT
  error_message TEXT
  created_at INTEGER NOT NULL
  updated_at INTEGER NOT NULL
```

Suggested statuses:

- `queued`
- `dispatching`
- `running`
- `waiting_input`
- `completed`
- `failed`
- `cancelled`

### 4.5 Task Events

Task events make long-running work visible and debuggable.

Suggested table:

```text
agent_task_events
  id TEXT PRIMARY KEY
  task_id TEXT NOT NULL
  event_type TEXT NOT NULL
  payload_json TEXT NOT NULL
  created_at INTEGER NOT NULL
```

Event types:

- `created`
- `dispatched`
- `status_changed`
- `log`
- `tool_call`
- `artifact_created`
- `memory_candidate`
- `completed`
- `failed`

### 4.6 Result Ingest

Result ingest converts external output back into AQBot-native state.

Responsibilities:

- Append final results to an AQBot conversation.
- Store structured task result payloads.
- Store artifacts using AQBot's existing documents root policy.
- Convert selected results into candidate memories.
- Keep execution logs available for review without cluttering normal chat.

Suggested module:

```text
src-tauri/src/external_agents/
  result_ingest.rs
```

Result ingest should reuse existing AQBot storage rules:

- Application state under `~/.aqbot/`.
- User-visible files under `~/Documents/aqbot/`.
- Stored file paths should be relative to the documents root.

### 4.7 Memory Candidate Ingest

External agents should not directly write long-term memory. They should return candidate memories.

Candidate memory shape:

```rust
pub struct MemoryCandidate {
    pub source_task_id: String,
    pub namespace: Option<String>,
    pub content: String,
    pub confidence: f32,
    pub reason: Option<String>,
}
```

AQBot then decides whether to:

- Save automatically under trusted rules.
- Ask the user for approval.
- Drop the candidate.
- Convert it into a knowledge-base document instead.

This keeps AQBot as the long-term memory authority.

## 5. Data Flows

### 5.1 User Sends Task to External Agent

```text
User action in AQBot
  -> create agent_tasks row
  -> build ExternalAgentTask
  -> connector.dispatch_task
  -> update status to running
  -> poll or receive callback
  -> ingest result
  -> append AQBot message
  -> optionally create memory candidates
```

### 5.2 External Channel Message Enters AQBot

For NanoClaw/OpenClaw-like channel services:

```text
External channel message
  -> external agent service
  -> AQBot callback or polling ingest
  -> AQBot creates/updates conversation
  -> AQBot decides route
  -> local or external execution
  -> response returns to channel through connector
```

AQBot should still record a conversation-level trace, even if the original channel is external.

### 5.3 Long-Running Task

```text
AQBot dispatches task
  -> external service starts long job
  -> events stream or poll into agent_task_events
  -> AQBot UI shows progress
  -> final result becomes message/artifact
```

### 5.4 Knowledge and Memory Context Injection

Before dispatching a task, AQBot may attach a context snapshot:

```text
Relevant memory items
Relevant knowledge snippets
Conversation summary
Selected files or artifacts
Task instructions
```

External services receive a snapshot, not direct ownership of AQBot's full knowledge store.

## 6. UI Surface

Keep the first UI small.

### 6.1 External Agents Settings

Add a settings section only when backend support exists.

Fields:

- Name.
- Kind.
- Base URL.
- Auth type.
- Auth value.
- Enabled.
- Capabilities.
- Test connection button.

### 6.2 Dispatch Entry

Add one entry point in chat or agent mode:

- "Send to external agent"
- Choose target agent.
- Optional task kind.
- Optional instruction override.

Do not replace the existing local Agent mode.

### 6.3 Task Status Panel

Initial version can be simple:

- Task title.
- Agent name.
- Status.
- Recent events.
- Error message.
- Result link.

Later it can become a dedicated tasks page.

## 7. Routing Strategy

Start with explicit user selection.

Phase 1:

- User chooses external agent manually.

Phase 2:

- Rule-based routing.

Example rules:

```text
local code work -> AQBot local Agent
channel message -> NanoClaw-like connector
scheduled summary -> automation worker
knowledge question -> AQBot local RAG/chat
container job -> external worker
```

Phase 3:

- Capability matching.

Connector capabilities may include:

- `channels.telegram`
- `channels.whatsapp`
- `filesystem.sandboxed`
- `automation.scheduled`
- `code.execution`
- `browser.automation`
- `long_task`

## 8. Implementation Plan

### Phase 0: Discovery and Guardrails

Tasks:

- Audit existing agent, memory, knowledge, and gateway boundaries.
- Confirm which current workflows must remain unchanged.
- Confirm submodule ownership and avoid unnecessary submodule edits.
- Add this document as the architectural baseline.

Deliverable:

- This design document.

### Phase 1: Backend Skeleton

Tasks:

- Add external agent types.
- Add migrations for `external_agents`, `agent_tasks`, and `agent_task_events`.
- Add repository methods for CRUD and status updates.
- Add Tauri commands:
  - `list_external_agents`
  - `create_external_agent`
  - `update_external_agent`
  - `delete_external_agent`
  - `dispatch_external_agent_task`
  - `list_agent_tasks`
  - `list_agent_task_events`

Deliverable:

- AQBot can store external agent definitions and create task records.

### Phase 2: Generic HTTP Connector

Tasks:

- Add `ExternalAgentConnector` trait.
- Add connector registry.
- Implement `custom_http_connector`.
- Support dispatch, status polling, and result fetching.
- Add connection test.

Deliverable:

- AQBot can call one generic external HTTP worker.

### Phase 3: Result Ingest

Tasks:

- Convert external results into AQBot messages.
- Store task events.
- Store external artifacts using AQBot documents root.
- Surface failures in the conversation.

Deliverable:

- External work appears inside AQBot as normal user-visible results.

### Phase 4: Minimal UI

Tasks:

- Add an external agents settings panel.
- Add a chat action to dispatch current prompt/context to an external agent.
- Add a task status view or compact task card.

Deliverable:

- User can configure and use external agents without editing config files.

### Phase 5: NanoClaw/OpenClaw Adapter

Tasks:

- Add a connector for NanoClaw-like services.
- Map NanoClaw session/task concepts to AQBot task records.
- Map NanoClaw result/log/artifact output to AQBot result ingest.
- Keep NanoClaw-specific types isolated in the connector module.

Deliverable:

- NanoClaw-like runtime can be used through AQBot without becoming AQBot's core runtime.

### Phase 6: Memory and Knowledge Loop

Tasks:

- Attach relevant AQBot memory and knowledge snippets to external tasks.
- Let connectors return memory candidates.
- Add approval or review flow for memory candidates.
- Add rules for when candidates can be auto-saved.

Deliverable:

- External task results can contribute to AQBot's long-term growth.

### Phase 7: Automation and Channels

Tasks:

- Add scheduled external tasks.
- Add external channel event ingest.
- Add route rules for channel-originated tasks.
- Add task summaries and recurring reports.

Deliverable:

- AQBot becomes a unified personal assistant console for chat, tasks, and external channels.

## 9. Main Repository vs Submodule Boundary

The guiding rule is:

> If AQBot can own the feature through a wrapper, bridge, connector, or adapter, keep it in the main repository.

Submodules should be changed only when a required behavior cannot be achieved from the main repository boundary.

### 9.1 Must Stay in AQBot Main Repository

These should be implemented in the main AQBot repository:

- External agent registry.
- External agent task tables.
- External task routing.
- Connector registry.
- Generic HTTP connector.
- NanoClaw/OpenClaw connector wrappers.
- Result ingest.
- Memory candidate ingest.
- Knowledge and memory context injection.
- External agent settings UI.
- Task status UI.
- Chat action for dispatching to external agents.
- AQBot conversation/message integration.
- Storage policies for returned files and artifacts.
- Permission and approval UI for external task dispatch.

Reason:

These are AQBot product features and should remain under AQBot's control.

### 9.2 Should Prefer Main Repository Wrappers

These may look like submodule concerns, but should first be attempted in AQBot wrappers:

- Mapping external task results into SDK messages.
- Adding provider-specific request metadata.
- Customizing how tool logs are displayed.
- Adding AQBot-specific agent context injection.
- Normalizing external tool-call records.
- Handling external agent errors.

Reason:

Wrapper code is easier to maintain than a forked submodule.

### 9.3 May Touch `open-agent-sdk`

Only consider modifying `src-tauri/crates/open-agent-sdk` if AQBot cannot express the needed behavior through its existing bridge.

Possible reasons:

- The SDK cannot preserve required reasoning/tool/message fields.
- The SDK cannot expose needed lifecycle hooks.
- The SDK cannot represent external task events cleanly.
- The SDK's context serialization blocks safe integration.

Avoid touching it for:

- External agent registry.
- Task database tables.
- UI task status.
- NanoClaw/OpenClaw connector logic.
- AQBot memory or knowledge storage.

Preferred alternative:

- Extend `src-tauri/crates/agent/src/bridge.rs`.
- Add AQBot-side adapter code.
- Keep SDK changes as a last resort.

### 9.4 May Touch `markstream-vue`

Only consider modifying `libs/markstream-vue` if result rendering cannot be achieved from AQBot's markdown node/component layer.

Possible reasons:

- A new message block type cannot be represented cleanly.
- Streaming rendering has a core parser limitation.
- Required markdown behavior must be fixed upstream.

Avoid touching it for:

- Task records.
- External agent dispatch.
- Status cards.
- Simple result messages.
- Memory candidate display.

Preferred alternative:

- Add AQBot-specific React components around existing markdown output.
- Encode external task events as AQBot display blocks before reaching the parser.

### 9.5 May Touch `stream-monaco`

Only consider modifying `libs/stream-monaco` if external agent artifacts require editor behavior that cannot be handled in AQBot.

Possible reasons:

- Streaming code editing needs new editor primitives.
- Artifact diff display requires a Monaco capability not exposed today.
- Inline execution annotations need editor-level support.

Avoid touching it for:

- Task dispatch.
- Task status.
- Agent connector implementation.
- Normal artifact storage.

Preferred alternative:

- Render task artifacts in AQBot components first.
- Use existing Monaco integration as-is.

### 9.6 Submodule Fork Policy

Fork a submodule only if all are true:

- The change is required.
- The change cannot be achieved through an AQBot wrapper.
- The change is expected to be long-lived.
- The upstream project is unlikely to accept or quickly release the change.

If a submodule fork is needed:

1. Fork the submodule repository separately.
2. Change that submodule remote to the fork.
3. Commit and push inside the submodule.
4. Return to AQBot main repository.
5. Commit the updated submodule pointer.

## 10. Risk Register

### 10.1 Over-Coupling to One External Runtime

Risk:

AQBot becomes a NanoClaw-specific shell.

Mitigation:

- Define AQBot-owned task contracts.
- Keep NanoClaw in a connector.
- Implement `custom_http_connector` first.

### 10.2 Memory Fragmentation

Risk:

Each external agent keeps its own memory and AQBot loses the user's unified assistant profile.

Mitigation:

- External agents submit memory candidates.
- AQBot owns final memory writes.
- AQBot injects context snapshots into tasks.

### 10.3 Submodule Maintenance Cost

Risk:

Deep submodule edits make syncing upstream difficult.

Mitigation:

- Prefer wrappers.
- Keep product features in the main repository.
- Touch submodules only for true lower-level limitations.

### 10.4 UI Complexity

Risk:

External agent features clutter chat UI.

Mitigation:

- Start with one dispatch action.
- Add a compact task card.
- Move advanced task management into a dedicated page later.

### 10.5 Security and Permission Drift

Risk:

External agents execute work with unclear permissions.

Mitigation:

- Store capabilities.
- Require user approval for high-risk dispatch.
- Show external agent identity and task scope.
- Avoid passing full memory/knowledge stores by default.

## 11. First Implementation Slice

The smallest useful first slice should be:

1. Add `external_agents` and `agent_tasks`.
2. Add a generic HTTP connector.
3. Add `dispatch_external_agent_task`.
4. Add result ingest that appends one assistant message to the current conversation.
5. Add a minimal settings UI for one external agent.

This slice proves the architecture without touching submodules.

## 12. Non-Goals for the First Slice

Do not implement these in the first slice:

- Full NanoClaw integration.
- Multi-channel inbox.
- Automatic memory writing.
- Complex route rules.
- New shortcut system.
- Submodule changes.
- Dedicated tasks dashboard.

These should come after the connector and task model prove themselves.
