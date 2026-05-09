import { create } from 'zustand';
import { invoke, listen, type UnlistenFn } from '@/lib/invoke';
import type {
  AgentDoneEvent,
  AgentLifecycleEvent,
  AgentProfile,
  AgentRun,
  AgentRunEvent,
  AgentSession,
  AgentStatusEvent,
  AskUserEvent,
  PermissionRequestEvent,
  ToolCallState,
  ToolResultEvent,
  ToolStartEvent,
  ToolUseEvent,
} from '@/types/agent';

interface QueryStats {
  numTurns?: number;
  inputTokens?: number;
  outputTokens?: number;
  costUsd?: number;
}

const EMPTY_RUN_EVENTS: AgentRunEvent[] = [];

interface AgentStore {
  profilesByConversation: Record<string, AgentProfile>;
  runsByConversation: Record<string, AgentRun[]>;
  runEventsByRunId: Record<string, AgentRunEvent[]>;
  activeRunIdByConversation: Record<string, string | undefined>;

  sessions: Record<string, AgentSession>;
  agentStatus: Record<string, string>;
  pendingPermissions: Record<string, PermissionRequestEvent>;
  pendingAskUser: Record<string, AskUserEvent>;
  toolCalls: Record<string, ToolCallState>;
  sdkIdToExecId: Record<string, string>;
  queryStats: Record<string, QueryStats>;

  fetchProfile: (conversationId: string) => Promise<AgentProfile | null>;
  fetchSession: (conversationId: string) => Promise<AgentSession | null>;
  updateProfile: (
    input: Partial<Pick<AgentProfile, 'workspaceRoot' | 'permissionMode' | 'defaultRunnerKind' | 'defaultProviderId' | 'defaultModelId'>> & {
      conversationId: string;
    },
  ) => Promise<AgentProfile | null>;
  updateCwd: (conversationId: string, cwd: string) => Promise<void>;
  updatePermissionMode: (conversationId: string, mode: string) => Promise<void>;
  fetchRuns: (conversationId: string) => Promise<AgentRun[]>;
  fetchRunEvents: (runId: string) => Promise<AgentRunEvent[]>;
  refreshConversationState: (conversationId: string) => Promise<void>;
  getLatestRun: (conversationId: string) => AgentRun | undefined;
  getActiveRun: (conversationId: string) => AgentRun | undefined;
  getRunTimeline: (runId: string) => AgentRunEvent[];
  canResumeRun: (runId: string) => boolean;
  canReplayRun: (runId: string) => boolean;
  approveToolUse: (conversationId: string, toolUseId: string, decision: string) => Promise<void>;
  respondAskUser: (askId: string, answer: string) => Promise<void>;
  cancelRun: (conversationId: string) => Promise<void>;
  resumeRun: (runId: string) => Promise<void>;

  handleToolUse: (event: ToolUseEvent) => void;
  handleToolStart: (event: ToolStartEvent) => void;
  handleToolResult: (event: ToolResultEvent) => void;
  handlePermissionRequest: (event: PermissionRequestEvent) => void;
  handlePermissionResolved: (toolUseId: string, decision: string) => void;
  handleAskUser: (event: AskUserEvent) => void;
  handleAskUserResolved: (askId: string) => void;
  handleStatus: (conversationId: string, message: string) => void;
  clearStatus: (conversationId: string) => void;
  handleDone: (event: AgentDoneEvent) => void;

  loadToolHistory: (conversationId: string) => Promise<void>;
  clearConversation: (conversationId: string) => void;
}

type EventPayload = Record<string, unknown>;

const ACTIVE_RUN_STATUSES = new Set([
  'queued',
  'starting',
  'running',
  'waiting_approval',
  'waiting_input',
  'cancelling',
]);

function parsePayload(payloadJson: string): EventPayload {
  try {
    return JSON.parse(payloadJson) as EventPayload;
  } catch {
    return {};
  }
}

function toCompatSession(profile: AgentProfile, run?: AgentRun): AgentSession {
  const runtimeStatus = run?.status && ACTIVE_RUN_STATUSES.has(run.status)
    ? (run.status as AgentSession['runtime_status'])
    : run?.status === 'completed'
      ? 'completed'
      : run?.status === 'failed' || run?.status === 'interrupted'
        ? 'error'
        : 'idle';

  let totalTokens = 0;
  if (run?.tokenUsageJson) {
    try {
      const usage = JSON.parse(run.tokenUsageJson) as { total_tokens?: number; input_tokens?: number; output_tokens?: number };
      totalTokens = usage.total_tokens ?? ((usage.input_tokens ?? 0) + (usage.output_tokens ?? 0));
    } catch {
      totalTokens = 0;
    }
  }

  return {
    id: profile.id,
    conversation_id: profile.conversationId,
    cwd: profile.workspaceRoot ?? undefined,
    permission_mode: profile.permissionMode as AgentSession['permission_mode'],
    runtime_status: runtimeStatus,
    total_tokens: totalTokens,
    total_cost_usd: run?.costUsd ?? 0,
  };
}

function projectConversationState(
  conversationId: string,
  runs: AgentRun[],
  runEventsByRunId: Record<string, AgentRunEvent[]>,
) {
  const toolCalls: Record<string, ToolCallState> = {};
  const pendingPermissions: Record<string, PermissionRequestEvent> = {};
  const pendingAskUser: Record<string, AskUserEvent> = {};
  const sdkIdToExecId: Record<string, string> = {};
  let activeRunId: string | undefined;
  let statusMessage: string | undefined;

  for (const run of runs) {
    if (!activeRunId && ACTIVE_RUN_STATUSES.has(run.status)) {
      activeRunId = run.id;
    }
    const events = runEventsByRunId[run.id] ?? [];
    for (const event of events) {
      const payload = parsePayload(event.payloadJson);
      switch (event.eventType) {
        case 'tool_use': {
          const toolUseId = String(payload.toolUseId ?? '');
          if (!toolUseId) break;
          const toolName = String(payload.toolName ?? '');
          const executionId = typeof payload.executionId === 'string' ? payload.executionId : undefined;
          const input = (payload.input as Record<string, unknown> | undefined) ?? {};
          const assistantMessageId = String(payload.assistantMessageId ?? '');
          const toolCall: ToolCallState = {
            toolUseId,
            toolName,
            input,
            assistantMessageId,
            executionStatus: 'queued',
            runId: run.id,
          };
          toolCalls[toolUseId] = toolCall;
          if (executionId) {
            toolCalls[executionId] = { ...toolCall, toolUseId: executionId };
            sdkIdToExecId[toolUseId] = executionId;
          }
          break;
        }
        case 'tool_start': {
          const toolUseId = String(payload.toolUseId ?? '');
          const existing = toolCalls[toolUseId];
          if (!existing) break;
          toolCalls[toolUseId] = { ...existing, executionStatus: 'running' };
          const execId = sdkIdToExecId[toolUseId];
          if (execId) toolCalls[execId] = { ...toolCalls[toolUseId], toolUseId: execId };
          break;
        }
        case 'tool_result': {
          const toolUseId = String(payload.toolUseId ?? '');
          const existing = toolCalls[toolUseId];
          const next: ToolCallState = {
            toolUseId,
            toolName: String(payload.toolName ?? existing?.toolName ?? ''),
            input: existing?.input ?? {},
            assistantMessageId: existing?.assistantMessageId ?? '',
            executionStatus: payload.isError ? 'failed' : 'success',
            approvalStatus: existing?.approvalStatus,
            output: typeof payload.content === 'string' ? payload.content : JSON.stringify(payload.content ?? ''),
            isError: Boolean(payload.isError),
            runId: run.id,
          };
          toolCalls[toolUseId] = next;
          const execId = sdkIdToExecId[toolUseId];
          if (execId) toolCalls[execId] = { ...next, toolUseId: execId };
          break;
        }
        case 'permission_request': {
          const toolUseId = String(payload.toolUseId ?? '');
          if (!toolUseId) break;
          pendingPermissions[toolUseId] = {
            conversationId,
            assistantMessageId: String(payload.assistantMessageId ?? toolCalls[toolUseId]?.assistantMessageId ?? ''),
            toolUseId,
            toolName: String(payload.toolName ?? toolCalls[toolUseId]?.toolName ?? ''),
            input: (payload.input as Record<string, unknown> | undefined) ?? {},
            riskLevel: 'execute',
          };
          break;
        }
        case 'permission_resolved': {
          const toolUseId = String(payload.toolUseId ?? '');
          if (toolUseId) {
            delete pendingPermissions[toolUseId];
            const existing = toolCalls[toolUseId];
            if (existing) {
              const approvalStatus = payload.decision === 'deny' ? 'denied' : 'approved';
              toolCalls[toolUseId] = { ...existing, approvalStatus };
              const execId = sdkIdToExecId[toolUseId];
              if (execId) toolCalls[execId] = { ...toolCalls[toolUseId], toolUseId: execId };
            }
          }
          break;
        }
        case 'ask_user': {
          const askId = String(payload.askId ?? '');
          if (!askId) break;
          pendingAskUser[askId] = {
            conversationId,
            assistantMessageId: String(payload.assistantMessageId ?? ''),
            askId,
            question: String(payload.question ?? ''),
            options: Array.isArray(payload.options) ? payload.options.map(String) : undefined,
          };
          break;
        }
        case 'ask_resolved': {
          const askId = String(payload.askId ?? '');
          if (askId) delete pendingAskUser[askId];
          break;
        }
        case 'status': {
          statusMessage = typeof payload.message === 'string' ? payload.message : statusMessage;
          break;
        }
        case 'run_interrupted': {
          statusMessage = 'Run interrupted';
          break;
        }
        default:
          break;
      }
    }
  }

  if (!activeRunId && runs.length > 0) {
    activeRunId = runs[0].id;
  }

  return {
    toolCalls,
    pendingPermissions,
    pendingAskUser,
    sdkIdToExecId,
    activeRunId,
    statusMessage,
  };
}

function latestRun(runs: AgentRun[] | undefined): AgentRun | undefined {
  return runs?.[0];
}

export const useAgentStore = create<AgentStore>((set, get) => ({
  profilesByConversation: {},
  runsByConversation: {},
  runEventsByRunId: {},
  activeRunIdByConversation: {},
  sessions: {},
  agentStatus: {},
  pendingPermissions: {},
  pendingAskUser: {},
  toolCalls: {},
  sdkIdToExecId: {},
  queryStats: {},

  fetchProfile: async (conversationId) => {
    try {
      const profile = await invoke<AgentProfile>('agent_get_profile', { conversationId });
      set((s) => {
        const latest = latestRun(s.runsByConversation[conversationId]);
        return {
          profilesByConversation: { ...s.profilesByConversation, [conversationId]: profile },
          sessions: { ...s.sessions, [conversationId]: toCompatSession(profile, latest) },
        };
      });
      return profile;
    } catch (e) {
      console.error('[agentStore] fetchProfile failed:', e);
      return null;
    }
  },

  fetchSession: async (conversationId) => {
    await get().refreshConversationState(conversationId);
    return get().sessions[conversationId] ?? null;
  },

  updateProfile: async (input) => {
    try {
      const profile = await invoke<AgentProfile>('agent_update_profile', {
        input: {
          conversationId: input.conversationId,
          workspaceRoot: input.workspaceRoot,
          permissionMode: input.permissionMode,
          defaultRunnerKind: input.defaultRunnerKind,
          defaultProviderId: input.defaultProviderId,
          defaultModelId: input.defaultModelId,
        },
      });
      set((s) => {
        const latest = latestRun(s.runsByConversation[input.conversationId]);
        return {
          profilesByConversation: {
            ...s.profilesByConversation,
            [input.conversationId]: profile,
          },
          sessions: {
            ...s.sessions,
            [input.conversationId]: toCompatSession(profile, latest),
          },
        };
      });
      return profile;
    } catch (e) {
      console.error('[agentStore] updateProfile failed:', e);
      return null;
    }
  },

  updateCwd: async (conversationId, cwd) => {
    await get().updateProfile({ conversationId, workspaceRoot: cwd });
  },

  updatePermissionMode: async (conversationId, mode) => {
    await get().updateProfile({ conversationId, permissionMode: mode });
  },

  fetchRuns: async (conversationId) => {
    try {
      const runs = await invoke<AgentRun[]>('agent_list_runs', { conversationId });
      set((s) => {
        const profile = s.profilesByConversation[conversationId];
        return {
          runsByConversation: { ...s.runsByConversation, [conversationId]: runs },
          activeRunIdByConversation: {
            ...s.activeRunIdByConversation,
            [conversationId]: runs.find((run) => ACTIVE_RUN_STATUSES.has(run.status))?.id ?? runs[0]?.id,
          },
          sessions: profile
            ? { ...s.sessions, [conversationId]: toCompatSession(profile, latestRun(runs)) }
            : s.sessions,
        };
      });
      return runs;
    } catch (e) {
      console.error('[agentStore] fetchRuns failed:', e);
      return [];
    }
  },

  fetchRunEvents: async (runId) => {
    try {
      const events = await invoke<AgentRunEvent[]>('agent_list_run_events', { runId });
      set((s) => ({
        runEventsByRunId: { ...s.runEventsByRunId, [runId]: events },
      }));
      return events;
    } catch (e) {
      console.error('[agentStore] fetchRunEvents failed:', e);
      return [];
    }
  },

  getLatestRun: (conversationId) => latestRun(get().runsByConversation[conversationId]),

  getActiveRun: (conversationId) => {
    const activeRunId = get().activeRunIdByConversation[conversationId];
    const runs = get().runsByConversation[conversationId] ?? [];
    return runs.find((run) => run.id === activeRunId)
      ?? runs.find((run) => ACTIVE_RUN_STATUSES.has(run.status));
  },

  getRunTimeline: (runId) => get().runEventsByRunId[runId] ?? EMPTY_RUN_EVENTS,

  canResumeRun: (runId) => {
    const run = Object.values(get().runsByConversation)
      .flat()
      .find((item) => item.id === runId);
    return run?.status === 'interrupted' && run.resumeCapability === 'resumable';
  },

  canReplayRun: (runId) => {
    const run = Object.values(get().runsByConversation)
      .flat()
      .find((item) => item.id === runId);
    return run?.status === 'interrupted' && run.resumeCapability === 'replay_only';
  },

  refreshConversationState: async (conversationId) => {
    const profile = await get().fetchProfile(conversationId);
    const runs = await get().fetchRuns(conversationId);
    const runEventsEntries = await Promise.all(
      runs.map(async (run) => [run.id, await get().fetchRunEvents(run.id)] as const),
    );
    const runEventsByRunId = Object.fromEntries(runEventsEntries);
    const projection = projectConversationState(conversationId, runs, runEventsByRunId);

    set((s) => ({
      runEventsByRunId: { ...s.runEventsByRunId, ...runEventsByRunId },
      toolCalls: {
        ...Object.fromEntries(
          Object.entries(s.toolCalls).filter(([, value]) => value.runId == null || !runs.some((run) => run.id === value.runId)),
        ),
        ...projection.toolCalls,
      },
      pendingPermissions: {
        ...Object.fromEntries(
          Object.entries(s.pendingPermissions).filter(([, value]) => value.conversationId !== conversationId),
        ),
        ...projection.pendingPermissions,
      },
      pendingAskUser: {
        ...Object.fromEntries(
          Object.entries(s.pendingAskUser).filter(([, value]) => value.conversationId !== conversationId),
        ),
        ...projection.pendingAskUser,
      },
      sdkIdToExecId: { ...s.sdkIdToExecId, ...projection.sdkIdToExecId },
      activeRunIdByConversation: {
        ...s.activeRunIdByConversation,
        [conversationId]: projection.activeRunId,
      },
      agentStatus: {
        ...s.agentStatus,
        ...(projection.statusMessage ? { [conversationId]: projection.statusMessage } : {}),
      },
      sessions:
        profile != null
          ? { ...s.sessions, [conversationId]: toCompatSession(profile, latestRun(runs)) }
          : s.sessions,
    }));
  },

  approveToolUse: async (conversationId, toolUseId, decision) => {
    try {
      const runId = get().activeRunIdByConversation[conversationId]
        ?? latestRun(get().runsByConversation[conversationId])?.id;
      if (!runId) return;
      await invoke('agent_control_run', {
        input: {
          runId,
          action: decision === 'deny' ? 'deny' : 'approve',
          targetId: toolUseId,
          value: decision,
          conversationId,
        },
      });
      get().handlePermissionResolved(toolUseId, decision);
    } catch (e) {
      console.error('[agentStore] approveToolUse failed:', e);
    }
  },

  respondAskUser: async (askId, answer) => {
    try {
      const ask = get().pendingAskUser[askId];
      if (!ask) return;
      const runId = get().activeRunIdByConversation[ask.conversationId]
        ?? latestRun(get().runsByConversation[ask.conversationId])?.id;
      if (!runId) return;
      await invoke('agent_control_run', {
        input: {
          runId,
          action: 'answer',
          targetId: askId,
          value: answer,
          conversationId: ask.conversationId,
        },
      });
      await new Promise((r) => setTimeout(r, 300));
      get().handleAskUserResolved(askId);
    } catch (e) {
      console.error('[agentStore] respondAskUser failed:', e);
    }
  },

  cancelRun: async (conversationId) => {
    try {
      const runId = get().activeRunIdByConversation[conversationId]
        ?? latestRun(get().runsByConversation[conversationId])?.id;
      if (!runId) return;
      await invoke('agent_control_run', {
        input: {
          runId,
          action: 'cancel',
          conversationId,
        },
      });
    } catch (e) {
      console.error('[agentStore] cancelRun failed:', e);
    }
  },

  resumeRun: async (runId) => {
    try {
      await invoke('agent_resume_run', { runId });
    } catch (e) {
      console.error('[agentStore] resumeRun failed:', e);
    }
  },

  handleToolUse: (event) => {
    set((s) => {
      const toolCall: ToolCallState = {
        toolUseId: event.toolUseId,
        toolName: event.toolName,
        input: event.input,
        assistantMessageId: event.assistantMessageId,
        executionStatus: 'queued',
      };
      const updates: Record<string, ToolCallState> = {
        [event.toolUseId]: toolCall,
      };
      const idMap = { ...s.sdkIdToExecId };
      if (event.executionId) {
        updates[event.executionId] = { ...toolCall, toolUseId: event.executionId };
        idMap[event.toolUseId] = event.executionId;
      }
      return {
        toolCalls: { ...s.toolCalls, ...updates },
        sdkIdToExecId: idMap,
      };
    });
  },

  handleToolStart: (event) => {
    set((s) => {
      const existing = s.toolCalls[event.toolUseId];
      const updated: ToolCallState = {
        toolUseId: event.toolUseId,
        toolName: event.toolName,
        input: event.input,
        assistantMessageId: event.assistantMessageId,
        executionStatus: 'running',
        approvalStatus: existing?.approvalStatus,
      };
      const updates: Record<string, ToolCallState> = {
        [event.toolUseId]: updated,
      };
      const execId = s.sdkIdToExecId[event.toolUseId];
      if (execId) {
        updates[execId] = { ...updated, toolUseId: execId };
      }
      return { toolCalls: { ...s.toolCalls, ...updates } };
    });
  },

  handleToolResult: (event) => {
    set((s) => {
      const existing = s.toolCalls[event.toolUseId];
      const newStatus = event.isError ? 'failed' : 'success';
      const updated: ToolCallState = {
        toolUseId: event.toolUseId,
        toolName: event.toolName || existing?.toolName || '',
        input: existing?.input ?? {},
        assistantMessageId: event.assistantMessageId,
        executionStatus: newStatus,
        approvalStatus: existing?.approvalStatus,
        output: event.content,
        isError: event.isError,
      };
      const updates: Record<string, ToolCallState> = {
        [event.toolUseId]: updated,
      };
      const execId = s.sdkIdToExecId[event.toolUseId];
      if (execId) {
        updates[execId] = { ...updated, toolUseId: execId };
      }
      return { toolCalls: { ...s.toolCalls, ...updates } };
    });
  },

  handlePermissionRequest: (event) => {
    set((s) => ({
      pendingPermissions: { ...s.pendingPermissions, [event.toolUseId]: event },
    }));
  },

  handlePermissionResolved: (toolUseId, decision) => {
    set((s) => {
      const { [toolUseId]: _removed, ...rest } = s.pendingPermissions;
      const existing = s.toolCalls[toolUseId];
      const approvalStatus: ToolCallState['approvalStatus'] =
        decision === 'deny' ? 'denied' : 'approved';
      const updatedToolCalls = existing
        ? {
            ...s.toolCalls,
            [toolUseId]: {
              ...existing,
              approvalStatus,
            },
          }
        : s.toolCalls;
      return {
        pendingPermissions: rest,
        toolCalls: updatedToolCalls,
      };
    });
  },

  handleAskUser: (event) => {
    set((s) => ({
      pendingAskUser: { ...s.pendingAskUser, [event.askId]: event },
    }));
  },

  handleAskUserResolved: (askId) => {
    set((s) => {
      const { [askId]: _removed, ...rest } = s.pendingAskUser;
      return { pendingAskUser: rest };
    });
  },

  handleStatus: (conversationId, message) => {
    set((s) => ({
      agentStatus: { ...s.agentStatus, [conversationId]: message },
    }));
  },

  clearStatus: (conversationId) => {
    set((s) => {
      const { [conversationId]: _removed, ...rest } = s.agentStatus;
      return { agentStatus: rest };
    });
  },

  handleDone: (event) => {
    const stats: QueryStats = {};
    if (event.numTurns != null) stats.numTurns = event.numTurns;
    if (event.usage) {
      stats.inputTokens = event.usage.input_tokens;
      stats.outputTokens = event.usage.output_tokens;
    }
    if (event.costUsd != null) stats.costUsd = event.costUsd;
    set((s) => ({
      queryStats: event.assistantMessageId && Object.keys(stats).length > 0
        ? { ...s.queryStats, [event.assistantMessageId]: stats }
        : s.queryStats,
    }));
  },

  loadToolHistory: async (conversationId) => {
    await get().refreshConversationState(conversationId);
  },

  clearConversation: (conversationId) => {
    set((s) => {
      const { [conversationId]: _session, ...sessions } = s.sessions;
      const { [conversationId]: _status, ...agentStatus } = s.agentStatus;
      const { [conversationId]: _profile, ...profilesByConversation } = s.profilesByConversation;
      const { [conversationId]: _runs, ...runsByConversation } = s.runsByConversation;
      const { [conversationId]: _activeRun, ...activeRunIdByConversation } = s.activeRunIdByConversation;

      const pendingPermissions: Record<string, PermissionRequestEvent> = {};
      for (const [id, pr] of Object.entries(s.pendingPermissions)) {
        if (pr.conversationId !== conversationId) pendingPermissions[id] = pr;
      }

      const pendingAskUser: Record<string, AskUserEvent> = {};
      for (const [id, ask] of Object.entries(s.pendingAskUser)) {
        if (ask.conversationId !== conversationId) pendingAskUser[id] = ask;
      }

      return {
        sessions,
        agentStatus,
        profilesByConversation,
        runsByConversation,
        activeRunIdByConversation,
        pendingPermissions,
        pendingAskUser,
      };
    });
  },
}));

export function setupAgentEventListeners(): () => void {
  const unlisteners: Promise<UnlistenFn>[] = [];
  const store = useAgentStore.getState();

  unlisteners.push(
    listen<ToolUseEvent>('agent-tool-use', (event) => {
      store.handleToolUse(event.payload);
    }),
  );

  unlisteners.push(
    listen<ToolStartEvent>('agent-tool-start', (event) => {
      store.handleToolStart(event.payload);
    }),
  );

  unlisteners.push(
    listen<ToolResultEvent>('agent-tool-result', (event) => {
      store.handleToolResult(event.payload);
    }),
  );

  unlisteners.push(
    listen<PermissionRequestEvent>('agent-permission-request', (event) => {
      store.handlePermissionRequest(event.payload);
    }),
  );

  unlisteners.push(
    listen<AskUserEvent>('agent-ask-user', (event) => {
      store.handleAskUser(event.payload);
    }),
  );

  unlisteners.push(
    listen<AgentStatusEvent>('agent-status', (event) => {
      store.handleStatus(event.payload.conversationId, event.payload.message);
    }),
  );

  unlisteners.push(
    listen<AgentDoneEvent>('agent-done', (event) => {
      const conversationId = event.payload.conversationId;
      store.clearStatus(conversationId);
      store.handleDone(event.payload);
      void useAgentStore.getState().refreshConversationState(conversationId);
    }),
  );

  unlisteners.push(
    listen<AgentLifecycleEvent>('agent-run-event', (event) => {
      const conversationId = event.payload.conversationId;
      if (!conversationId) return;
      if (event.payload.message) {
        store.handleStatus(conversationId, event.payload.message);
      }
      void useAgentStore.getState().refreshConversationState(conversationId);
    }),
  );

  return () => {
    for (const p of unlisteners) {
      p.then((u) => u());
    }
  };
}
