import { create } from 'zustand';
import { invoke } from '@/lib/invoke';
import type {
  AgentTask,
  AgentTaskEvent,
  CreateExternalAgentInput,
  DispatchExternalAgentTaskInput,
  DispatchExternalAgentTaskResult,
  ExternalAgent,
  ExternalAgentConnectionTestResult,
  UpdateExternalAgentInput,
} from '@/types';

interface ExternalAgentState {
  agents: ExternalAgent[];
  tasks: AgentTask[];
  loading: boolean;
  error: string | null;
  loadAgents: () => Promise<void>;
  createAgent: (input: CreateExternalAgentInput) => Promise<ExternalAgent>;
  updateAgent: (id: string, input: UpdateExternalAgentInput) => Promise<ExternalAgent>;
  deleteAgent: (id: string) => Promise<void>;
  testAgent: (id: string) => Promise<ExternalAgentConnectionTestResult>;
  dispatchTask: (input: DispatchExternalAgentTaskInput) => Promise<DispatchExternalAgentTaskResult>;
  retryTask: (taskId: string) => Promise<DispatchExternalAgentTaskResult>;
  syncTask: (taskId: string) => Promise<DispatchExternalAgentTaskResult>;
  loadTasks: (filters?: { conversationId?: string; externalAgentId?: string; limit?: number }) => Promise<void>;
  listTaskEvents: (taskId: string) => Promise<AgentTaskEvent[]>;
}

export const useExternalAgentStore = create<ExternalAgentState>((set) => ({
  agents: [],
  tasks: [],
  loading: false,
  error: null,

  loadAgents: async () => {
    set({ loading: true });
    try {
      const agents = await invoke<ExternalAgent[]>('list_external_agents');
      set({ agents, loading: false, error: null });
    } catch (e) {
      set({ loading: false, error: String(e) });
    }
  },

  createAgent: async (input) => {
    const agent = await invoke<ExternalAgent>('create_external_agent', { input });
    set((state) => ({ agents: [...state.agents, agent], error: null }));
    return agent;
  },

  updateAgent: async (id, input) => {
    const agent = await invoke<ExternalAgent>('update_external_agent', { id, input });
    set((state) => ({
      agents: state.agents.map((item) => (item.id === id ? agent : item)),
      error: null,
    }));
    return agent;
  },

  deleteAgent: async (id) => {
    await invoke('delete_external_agent', { id });
    set((state) => ({
      agents: state.agents.filter((item) => item.id !== id),
      tasks: state.tasks.filter((item) => item.externalAgentId !== id),
      error: null,
    }));
  },

  testAgent: async (id) => {
    return invoke<ExternalAgentConnectionTestResult>('test_external_agent_connection', { id });
  },

  dispatchTask: async (input) => {
    const result = await invoke<DispatchExternalAgentTaskResult>('dispatch_external_agent_task', { input });
    set((state) => ({ tasks: [result.task, ...state.tasks], error: null }));
    return result;
  },

  retryTask: async (taskId) => {
    const result = await invoke<DispatchExternalAgentTaskResult>('retry_external_agent_task', { taskId });
    set((state) => ({ tasks: [result.task, ...state.tasks], error: null }));
    return result;
  },

  syncTask: async (taskId) => {
    const result = await invoke<DispatchExternalAgentTaskResult>('sync_external_agent_task', { taskId });
    set((state) => ({
      tasks: state.tasks.map((task) => (task.id === taskId ? result.task : task)),
      error: null,
    }));
    return result;
  },

  loadTasks: async (filters) => {
    const tasks = await invoke<AgentTask[]>('list_agent_tasks', {
      conversationId: filters?.conversationId,
      externalAgentId: filters?.externalAgentId,
      limit: filters?.limit,
    });
    set({ tasks, error: null });
  },

  listTaskEvents: async (taskId) => {
    return invoke<AgentTaskEvent[]>('list_agent_task_events', { taskId });
  },
}));
