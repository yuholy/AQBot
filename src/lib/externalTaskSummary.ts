import type { AgentTask } from '@/types';

function toDisplayString(value: unknown): string | null {
  if (typeof value === 'string') {
    const trimmed = value.trim();
    return trimmed || null;
  }
  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }
  return null;
}

function pickSummaryCandidate(payload: unknown): string | null {
  if (!payload || typeof payload !== 'object') {
    return null;
  }

  const record = payload as Record<string, unknown>;
  const directKeys = ['summary', 'message', 'content', 'text', 'answer', 'finalText', 'output'];
  for (const key of directKeys) {
    const picked = toDisplayString(record[key]);
    if (picked) {
      return picked;
    }
  }

  const nestedKeys = ['result', 'data', 'task', 'response'];
  for (const key of nestedKeys) {
    const picked = pickSummaryCandidate(record[key]);
    if (picked) {
      return picked;
    }
  }

  if (Array.isArray(record.artifacts) && record.artifacts.length > 0) {
    return `${record.artifacts.length} artifact${record.artifacts.length > 1 ? 's' : ''} generated`;
  }

  return null;
}

export function getExternalTaskSummary(task: AgentTask): string {
  if (task.errorMessage?.trim()) {
    return task.errorMessage.trim();
  }

  if (task.resultPayloadJson?.trim()) {
    try {
      const parsed = JSON.parse(task.resultPayloadJson);
      const candidate = pickSummaryCandidate(parsed);
      if (candidate) {
        return candidate.replace(/\s+/g, ' ').trim();
      }
    } catch {
      return task.resultPayloadJson.trim();
    }
  }

  return task.kind;
}
