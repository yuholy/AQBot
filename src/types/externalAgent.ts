import type { Message } from './index';

export interface ExternalAgent {
  id: string;
  name: string;
  kind: string;
  baseUrl?: string | null;
  authType: string;
  authConfigJson?: string | null;
  capabilitiesJson: string;
  enabled: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface CreateExternalAgentInput {
  name: string;
  kind: string;
  baseUrl?: string | null;
  authType?: string;
  authConfigJson?: string | null;
  capabilitiesJson?: string | null;
  enabled?: boolean;
}

export interface UpdateExternalAgentInput {
  name?: string;
  kind?: string;
  baseUrl?: string | null;
  authType?: string;
  authConfigJson?: string | null;
  capabilitiesJson?: string;
  enabled?: boolean;
}

export interface AgentTask {
  id: string;
  conversationId?: string | null;
  sourceMessageId?: string | null;
  externalAgentId: string;
  externalTaskId?: string | null;
  kind: string;
  status: string;
  title: string;
  requestPayloadJson: string;
  resultPayloadJson?: string | null;
  errorMessage?: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface AgentTaskEvent {
  id: string;
  taskId: string;
  eventType: string;
  payloadJson: string;
  createdAt: number;
}

export interface DispatchExternalAgentTaskInput {
  conversationId?: string | null;
  sourceMessageId?: string | null;
  externalAgentId: string;
  kind?: string;
  title: string;
  inputText: string;
  contextJson?: string | null;
}

export interface DispatchExternalAgentTaskResult {
  task: AgentTask;
  assistantMessage?: Message | null;
}

export interface ExternalAgentConnectionTestResult {
  ok: boolean;
  status?: number | null;
  message?: string | null;
}
