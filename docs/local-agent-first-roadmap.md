# AQBot Local-Agent-First Roadmap

## 1. Decision

AQBot should evolve as a local-agent-first personal assistant.

For the input-bar executor selector design, see
[Agent Executor Selector Design](./agent-executor-selector-design.md).

Remote agents such as OpenClaw are useful, but they should be optional execution
providers rather than the main AQBot experience.

Target positioning:

```text
AQBot local agent = primary assistant and control plane
External agents = optional pluggable execution providers
```

## 2. Why Local First

AQBot is a desktop personal assistant. Its strongest assets are local:

- Conversation history.
- User files.
- Knowledge bases.
- Memory.
- Skills.
- Provider and model configuration.
- Desktop UI state.
- Local project folders.
- User preferences.

A local primary agent can use these directly and provide a smoother experience
than routing ordinary tasks through a remote task system.

## 3. Role Split

### Local Agent

Use for:

- Default Agent mode in AQBot chat.
- Local project work.
- Knowledge and memory assisted tasks.
- File organization.
- Personal workflows.
- AQBot-native tools.
- Skills and MCP usage.

### Remote OpenClaw

Use for:

- Long-running jobs.
- Remote Linux/server operations.
- Always-on channel bots.
- DingTalk/Feishu style integrations.
- Tasks that should continue when the desktop app is closed.
- Specialized external agent runtimes.

Remote OpenClaw should stay in the connector system as an experimental or
advanced connector, not as the default chat execution path.

## 4. Product Principles

- Normal chat should never look like an API task console.
- Agent mode should feel like "AQBot is working", not "I am managing tasks".
- External connector details belong in settings and diagnostics.
- Results should flow back as normal assistant messages.
- Task ids, raw payloads, Sync, Retry, and event logs are diagnostics features.

## 5. Immediate Cleanup

Done or planned:

- Keep external-agent tables and connector code.
- Keep OpenClaw connector configuration in Settings.
- Remove persistent external-task banners from the chat viewport.
- Remove external-agent launch controls from the primary chat header.
- Keep external-agent task management in diagnostics/settings.

## 6. Local Agent V1 Shape

Local Agent V1 should provide a single native AQBot execution path:

```text
User request
  -> AQBot local planner
  -> AQBot context collector
  -> AQBot tool executor
  -> AQBot result renderer
  -> AQBot memory / knowledge / file ingest
```

### 6.1 Planner

Responsibilities:

- Decide whether a request is simple Q&A or agentic execution.
- Build a small task plan when needed.
- Ask for confirmation before risky operations.
- Prefer local context before external providers.

### 6.2 Context Collector

Responsibilities:

- Pull recent conversation messages.
- Retrieve memory snippets.
- Retrieve knowledge snippets.
- Attach selected files.
- Include current project/workspace context.

### 6.3 Tool Executor

Responsibilities:

- Execute approved local tools.
- Read and write user-visible files under the documents root.
- Use project workspaces safely.
- Call MCP tools where configured.
- Dispatch to external agents only when the task requires remote execution.

### 6.4 Result Renderer

Responsibilities:

- Show progress in chat.
- Render final results as assistant messages.
- Show artifacts as normal attachments or file-center entries.
- Keep diagnostics behind an explicit drawer.

## 7. External Agent Role After Repositioning

External agent support remains valuable because it becomes the plugin boundary.

It should support:

- OpenClaw connector.
- NanoClaw connector.
- Custom HTTP connector.
- Future Dify/Coze/LangGraph/CrewAI connectors.

But the default path should be:

```text
AQBot local Agent first
External connector only when selected or routed by policy
```

## 8. Implementation Plan

### Phase 1: Hide Experimental Surface

- Remove external-agent controls from the main chat header.
- Keep Settings -> External Agents.
- Keep task diagnostics in the settings page.
- Do not delete connector code.

### Phase 2: Define Local Agent Runtime

- Audit the existing built-in agent mode.
- Identify reusable pieces:
  - context collection
  - tool calling
  - permission cards
  - file attachment handling
  - model invocation
- Define the local agent run table if existing records are not enough.

### Phase 3: Build Local Agent UX

- Make Ask/Agent mode clearer.
- Add an "AQBot is working" progress surface.
- Insert local agent results as normal assistant messages.
- Keep diagnostics optional.

### Phase 4: Add Routing Policy

- Let users choose:
  - Local AQBot Agent
  - Remote OpenClaw
  - Other configured connector
- Add rules later:
  - "Use remote OpenClaw for server tasks"
  - "Use local agent for file and knowledge tasks"

### Phase 5: Revisit OpenClaw Connector

- Keep it as a connector implementation.
- Make setup clearer.
- Remove any assumption that it is the primary agent.
- Add better diagnostics and status mapping.

## 9. Non-Goals

- Deleting the OpenClaw integration immediately.
- Rebuilding AQBot around a remote agent runtime.
- Showing external task internals in the normal chat surface.
- Making every connector share AQBot memory automatically.
