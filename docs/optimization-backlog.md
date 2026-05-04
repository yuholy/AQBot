# AQBot Optimization Backlog

This document tracks product and engineering improvements that are worth doing,
but do not need to block the current mainline.

## P0

- Move AQBot toward the local-agent-first roadmap.
  The remote OpenClaw integration is useful as a connector, but the main product
  experience should be AQBot's native local agent. See
  `docs/local-agent-first-roadmap.md` and
  `docs/agent-executor-selector-design.md`.

  Scope:
  - Keep OpenClaw as an optional connector.
  - Keep Settings -> External Agents for setup and diagnostics.
  - Remove external-agent controls from the main chat header.
  - Design the input-bar Agent executor selector before adding more executors.
  - Audit the existing built-in Agent mode and define the local runtime path.
  - Reuse the connector standard only for external providers.

- Productize the external-agent experience around the connector standard.
  AQBot can already dispatch tasks to OpenClaw-compatible services, but the
  interaction still feels like an engineering task console. The next step is to
  make agents feel like a coherent execution mode inside chat.

  Scope:
  - Follow `docs/agent-connector-standard.md`.
  - Keep raw Sync/Retry/event controls in diagnostics, not in the main chat
    viewport.
  - Insert completed external-agent results as normal assistant messages.
  - Show only lightweight progress/failure states in conversation.
  - Keep connector-specific details behind the connector boundary.

- Improve vector model onboarding for knowledge base and memory.
  The current experience fails silently when no embedding-capable model exists.
  Users can open the "add knowledge base" or "add namespace" dialog and see an
  empty vector-model selector with no clear next step.

  Scope:
  - Detect when no embedding models are available.
  - Replace empty dropdown states with explicit guidance.
  - Add direct navigation or inline actions to provider/model configuration.
  - Show which prerequisite is missing:
    - no enabled provider
    - no API key
    - no embedding model
    - model exists but is disabled

- Expose custom provider creation in the Provider UI.
  The codebase already supports `custom` provider behavior in the lower layers,
  but the "add provider" UI currently does not offer `custom` as a selectable
  provider type.

  Scope:
  - Add `custom` to the provider-type selector.
  - Provide a guided form for custom OpenAI-compatible endpoints.
  - Support API host, optional API path, and custom headers cleanly.
  - Add validation and user-facing examples.

## P1

- Support a smoother local embedding setup path.
  AQBot should make it practical to connect local embedding services instead of
  assuming cloud-only vectorization.

  Scope:
  - Document the supported OpenAI-compatible local endpoint pattern.
  - Provide a preset flow for common local embedding backends.
  - Allow quick creation of a local embedding provider from the UI.
  - Pre-fill sensible defaults for host, path, and model-type selection.

- Improve embedding model management inside provider detail.
  Today users must understand provider, model, and model type concepts before
  they can make knowledge or memory work.

  Scope:
  - Add a one-click "Add Embedding Model" shortcut.
  - Recommend common model IDs for supported providers.
  - Warn when a provider only has chat/image models.
  - Surface which models are currently usable by knowledge base and memory.

- Evolve the Files page into a practical personal file center.
  The current Files page mainly acts as a passive attachment registry, so users
  do not clearly understand why it exists or how it helps their workflow.

  Scope:
  - Clarify the purpose of the page as a unified asset center.
  - Show file origin, such as conversation, knowledge base, or generated output.
  - Add direct actions like open, reveal, attach to chat, and add to knowledge base.
  - Support explicit upload/import instead of only showing passively created files.
  - Add cleanup and orphan-file management views.

- Reduce chat conversation switch latency.
  Switching between conversations currently feels heavy because the UI clears
  the current message list first, then reloads the next conversation from
  scratch.

  Scope:
  - Reuse recent conversation message snapshots during navigation.
  - Refresh in the background after the cached content is shown.
  - Audit expensive per-switch work in ChatView and defer non-critical work.
  - Avoid visible blank states when switching among recently opened chats.

## P2

- Add a no-embedding fallback mode for knowledge and memory.
  This is a product downgrade path for users who do not have an embedding model
  yet but still want a basic usable system.

  Candidate directions:
  - keyword search fallback for knowledge base
  - exact/substring retrieval for memory items
  - clear UX distinction between "basic retrieval" and "vector retrieval"

  Notes:
  - This should be treated as a usability bridge, not a replacement for vector
    search.
  - The fallback should not complicate the normal embedding-enabled path.

- Add an explicit setup checklist for knowledge and memory readiness.
  Users should be able to answer "why is this unavailable?" without guessing.

  Scope:
  - Provider ready
  - API key ready
  - Embedding model ready
  - Index built
  - Chat context attached

## Notes

- Current confirmed user pain:
  - no visible path to configure vector models when none exist
  - empty vector-model dropdown feels broken
  - local embedding setup is unclear
  - custom provider support is partially implemented but not fully exposed

- Recommended implementation order:
  1. Empty-state guidance for vector model selection
  2. Custom provider UI exposure
  3. Local embedding setup presets
  4. Optional no-embedding fallback
