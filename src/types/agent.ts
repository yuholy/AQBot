export type AgentPermissionMode = 'default' | 'accept_edits' | 'full_access';
export type AgentRuntimeStatus = 'idle' | 'running' | 'waiting_approval' | 'completed' | 'error';
export type ApprovalStatus = 'pending' | 'approved' | 'denied';
export type ResumeCapability = 'none' | 'replay_only' | 'resumable';
export type AgentRunStatus =
  | 'queued'
  | 'starting'
  | 'running'
  | 'waiting_approval'
  | 'waiting_input'
  | 'cancelling'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'interrupted';

export interface AgentProfile {
  id: string;
  conversationId: string;
  workspaceRoot?: string | null;
  permissionMode: AgentPermissionMode | string;
  defaultRunnerKind: string;
  defaultProviderId?: string | null;
  defaultModelId?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface AgentRun {
  id: string;
  conversationId: string;
  profileId: string;
  runnerKind: string;
  providerId?: string | null;
  modelId?: string | null;
  status: AgentRunStatus | string;
  promptSnapshot: string;
  sdkContextJson?: string | null;
  workspaceRoot?: string | null;
  startedAt: string;
  finishedAt?: string | null;
  errorSummary?: string | null;
  tokenUsageJson?: string | null;
  costUsd: number;
  resumeCapability: ResumeCapability | string;
  interruptedReason?: string | null;
  resumeTokenJson?: string | null;
}

export interface AgentRunEvent {
  id: string;
  runId: string;
  stepId?: string | null;
  eventType: string;
  payloadJson: string;
  sequenceNo: number;
  createdAt: string;
}

export interface AgentSession {
  id: string;
  conversation_id: string;
  cwd?: string;
  permission_mode: AgentPermissionMode;
  runtime_status: AgentRuntimeStatus;
  total_tokens: number;
  total_cost_usd: number;
}

// --- Event payload types (all tool-related events carry assistantMessageId anchor) ---

export interface ToolUseEvent {
  conversationId: string;
  assistantMessageId: string;
  toolUseId: string;
  toolName: string;
  input: Record<string, unknown>;
  executionId?: string;
}

export interface ToolStartEvent {
  conversationId: string;
  assistantMessageId: string;
  toolUseId: string;
  toolName: string;
  input: Record<string, unknown>;
}

export interface ToolResultEvent {
  conversationId: string;
  assistantMessageId: string;
  toolUseId: string;
  toolName: string;
  content: string;
  isError: boolean;
}

export interface PermissionRequestEvent {
  conversationId: string;
  assistantMessageId: string;
  toolUseId: string;
  toolName: string;
  input: Record<string, unknown>;
  riskLevel: 'read_only' | 'write' | 'execute';
}

export interface AskUserEvent {
  conversationId: string;
  assistantMessageId: string;
  askId: string;
  question: string;
  options?: string[];
}

export interface AgentDoneEvent {
  conversationId: string;
  assistantMessageId: string;
  text: string;
  thinking?: string;
  model?: string;
  sessionId?: string;
  usage?: { input_tokens: number; output_tokens: number };
  numTurns?: number;
  costUsd?: number;
}

export interface AgentLifecycleEvent {
  conversationId?: string;
  runId?: string;
  status?: AgentRunStatus | string;
  resumeCapability?: ResumeCapability | string;
  interruptedReason?: string;
  assistantMessageId?: string;
  text?: string;
  message?: string;
}

export interface AgentErrorEvent {
  conversationId: string;
  assistantMessageId?: string;
  message: string;
}

export interface AgentCancelledEvent {
  conversationId: string;
  assistantMessageId?: string;
  reason: string;
}

export interface AgentStatusEvent {
  conversationId: string;
  message: string;
}

export interface AgentRateLimitEvent {
  conversationId: string;
  retryAfterMs: number;
  message: string;
}

export interface AgentStreamTextEvent {
  conversationId: string;
  assistantMessageId: string;
  text: string;
}

export interface AgentStreamThinkingEvent {
  conversationId: string;
  assistantMessageId: string;
  thinking: string;
}

// --- Frontend runtime state ---

export interface ToolCallState {
  toolUseId: string;
  toolName: string;
  input: Record<string, unknown>;
  assistantMessageId: string;
  executionStatus: 'queued' | 'running' | 'success' | 'failed' | 'cancelled';
  approvalStatus?: ApprovalStatus;
  output?: string;
  isError?: boolean;
  runId?: string;
}
