import { useMemo } from 'react';
import { Tabs, Empty, List, Descriptions, Tag, Typography, theme } from 'antd';
import { Search, Wrench, Paperclip, Info, FileText, Activity } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useConversationStore, useArtifactStore, useAgentStore } from '@/stores';

const EMPTY_AGENT_RUNS: readonly [] = [];
const EMPTY_AGENT_RUN_EVENTS: readonly [] = [];

interface ChatInspectorProps {
  visible: boolean;
  activeTab: string;
  onTabChange: (tab: string) => void;
  conversationId: string | null;
}

export function ChatInspector({
  visible,
  activeTab,
  onTabChange,
  conversationId,
}: ChatInspectorProps) {
  const { t } = useTranslation();
  const { token } = theme.useToken();

  const conversation = useConversationStore((s) =>
    s.conversations.find((c) => c.id === s.activeConversationId),
  );
  const workspaceSnapshot = useConversationStore((s) => s.workspaceSnapshot);
  const messages = useConversationStore((s) => s.messages);
  const { artifacts } = useArtifactStore();
  const agentProfile = useAgentStore((s) =>
    conversationId ? s.profilesByConversation[conversationId] : undefined,
  );
  const agentRuns = useAgentStore((s) =>
    conversationId ? s.runsByConversation[conversationId] ?? EMPTY_AGENT_RUNS : EMPTY_AGENT_RUNS,
  );
  const agentRunEvents = useAgentStore((s) => {
    if (!conversationId) return EMPTY_AGENT_RUN_EVENTS;
    const latestRunId = s.runsByConversation[conversationId]?.[0]?.id;
    return latestRunId ? s.runEventsByRunId[latestRunId] ?? EMPTY_AGENT_RUN_EVENTS : EMPTY_AGENT_RUN_EVENTS;
  });
  const latestRun = agentRuns[0];

  const contextSources = useMemo(() => {
    if (!workspaceSnapshot) return [];
    const sources: { type: string; title: string }[] = [];
    if (workspaceSnapshot.knowledgeBinding?.knowledgeBaseIds?.length) {
      workspaceSnapshot.knowledgeBinding.knowledgeBaseIds.forEach((id) =>
        sources.push({ type: 'knowledge', title: id }),
      );
    }
    if (workspaceSnapshot.searchPolicy?.enabled) {
      sources.push({ type: 'search', title: workspaceSnapshot.searchPolicy.searchProviderId ?? 'search' });
    }
    if (workspaceSnapshot.memoryPolicy?.enabled) {
      sources.push({ type: 'memory', title: workspaceSnapshot.memoryPolicy.namespaceId ?? 'memory' });
    }
    if (workspaceSnapshot.toolBinding?.serverIds?.length) {
      workspaceSnapshot.toolBinding.serverIds.forEach((id) =>
        sources.push({ type: 'tool', title: id }),
      );
    }
    return sources;
  }, [workspaceSnapshot]);

  const toolCalls = useMemo(() => {
    return agentRunEvents
      .filter((event) => event.eventType === 'tool_use' || event.eventType === 'tool_result')
      .map((event) => {
        let payload: Record<string, unknown> = {};
        try {
          payload = JSON.parse(event.payloadJson) as Record<string, unknown>;
        } catch {
          payload = {};
        }
        return {
          name: String(payload.toolName ?? event.eventType),
          messageId: String(payload.assistantMessageId ?? event.runId),
        };
      });
  }, [agentRunEvents]);

  const conversationArtifacts = useMemo(() => {
    if (!conversationId) return [];
    return artifacts.filter((a) => a.conversationId === conversationId);
  }, [artifacts, conversationId]);

  const runTimeline = useMemo(() => {
    return agentRunEvents.map((event) => {
      let payload: Record<string, unknown> = {};
      try {
        payload = JSON.parse(event.payloadJson) as Record<string, unknown>;
      } catch {
        payload = {};
      }
      const title = String(payload.toolName ?? payload.message ?? event.eventType);
      return {
        id: event.id,
        type: event.eventType,
        title,
        createdAt: event.createdAt,
      };
    });
  }, [agentRunEvents]);

  const tabItems = useMemo(
    () => [
      {
        key: 'sources',
        label: t('chat.inspector.sources'),
        icon: <Search size={14} />,
        children: contextSources.length > 0 ? (
          <List
            size="small"
            dataSource={contextSources}
            renderItem={(item) => (
              <List.Item>
                <List.Item.Meta
                  title={<Typography.Text>{item.title}</Typography.Text>}
                  description={<Tag>{item.type}</Tag>}
                />
              </List.Item>
            )}
          />
        ) : (
          <Empty
            description={conversationId ? t('common.noData') : t('common.noData')}
            style={{ marginTop: 48 }}
          />
        ),
      },
      {
        key: 'tools',
        label: t('chat.inspector.tools'),
        icon: <Wrench size={14} />,
        children: toolCalls.length > 0 ? (
          <List
            size="small"
            dataSource={toolCalls}
            renderItem={(item) => (
              <List.Item>
                <Typography.Text code>{item.name}</Typography.Text>
              </List.Item>
            )}
          />
        ) : (
          <Empty
            description={t('chat.inspector.tools')}
            style={{ marginTop: 48 }}
          />
        ),
      },
      {
        key: 'attachments',
        label: t('chat.inspector.attachments'),
        icon: <Paperclip size={14} />,
        children: (() => {
          const attachments = messages.flatMap((m) => m.attachments ?? []);
          return attachments.length > 0 ? (
            <List
              size="small"
              dataSource={attachments}
              renderItem={(item) => (
                <List.Item>
                  <Typography.Text ellipsis>{item.file_name}</Typography.Text>
                </List.Item>
              )}
            />
          ) : (
            <Empty
              description={t('chat.inspector.attachments')}
              style={{ marginTop: 48 }}
            />
          );
        })(),
      },
      {
        key: 'session',
        label: t('chat.inspector.session'),
        icon: <Info size={14} />,
        children: conversation ? (
          <Descriptions column={1} size="small" style={{ padding: '8px 0' }}>
            <Descriptions.Item label={t('chat.inspector.session')}>
              <Typography.Text copyable={{ text: conversation.id }}>
                {conversation.id.slice(0, 8)}…
              </Typography.Text>
            </Descriptions.Item>
            <Descriptions.Item label={t('gateway.defaultProvider')}>
              {agentRuns[0]?.providerId || conversation.provider_id || '-'}
            </Descriptions.Item>
            <Descriptions.Item label={t('gateway.defaultModel')}>
              {agentRuns[0]?.modelId || conversation.model_id || '-'}
            </Descriptions.Item>
            <Descriptions.Item label={t('common.status', '状态')}>
              {agentRuns[0]?.status || '-'}
            </Descriptions.Item>
            <Descriptions.Item label={t('common.permission', '权限')}>
              {agentProfile?.permissionMode || '-'}
            </Descriptions.Item>
            <Descriptions.Item label={t('gateway.created')}>
              {new Date(conversation.created_at).toLocaleString()}
            </Descriptions.Item>
            <Descriptions.Item label={t('chat.inspector.tools')}>
              {conversation.message_count}
            </Descriptions.Item>
          </Descriptions>
        ) : (
          <Empty
            description={t('common.noData')}
            style={{ marginTop: 48 }}
          />
        ),
      },
      {
        key: 'run',
        label: 'Run',
        icon: <Activity size={14} />,
        children: latestRun ? (
          <div style={{ padding: '8px 0' }}>
            <Descriptions column={1} size="small">
              <Descriptions.Item label="Status">
                <Tag>{latestRun.status}</Tag>
              </Descriptions.Item>
              <Descriptions.Item label="Runner">
                {latestRun.runnerKind}
              </Descriptions.Item>
              <Descriptions.Item label="Resume">
                {latestRun.resumeCapability || 'none'}
              </Descriptions.Item>
              <Descriptions.Item label="Interrupted">
                {latestRun.interruptedReason || '-'}
              </Descriptions.Item>
              <Descriptions.Item label="Started">
                {latestRun.startedAt}
              </Descriptions.Item>
              <Descriptions.Item label="Finished">
                {latestRun.finishedAt || '-'}
              </Descriptions.Item>
              <Descriptions.Item label="Cost">
                {latestRun.costUsd ? `$${latestRun.costUsd.toFixed(4)}` : '-'}
              </Descriptions.Item>
              <Descriptions.Item label="Error">
                {latestRun.errorSummary || '-'}
              </Descriptions.Item>
            </Descriptions>
            <List
              size="small"
              dataSource={runTimeline}
              style={{ marginTop: 12 }}
              renderItem={(item) => (
                <List.Item>
                  <List.Item.Meta
                    title={<Typography.Text>{item.type}</Typography.Text>}
                    description={
                      <Typography.Text type="secondary" ellipsis>
                        {item.title}
                      </Typography.Text>
                    }
                  />
                </List.Item>
              )}
            />
          </div>
        ) : (
          <Empty
            description={t('common.noData')}
            style={{ marginTop: 48 }}
          />
        ),
      },
      {
        key: 'artifacts',
        label: t('chat.inspector.artifacts'),
        icon: <FileText size={14} />,
        children: conversationArtifacts.length > 0 ? (
          <List
            size="small"
            dataSource={conversationArtifacts}
            renderItem={(item) => (
              <List.Item>
                <List.Item.Meta
                  title={<Typography.Text>{item.title}</Typography.Text>}
                  description={<Tag>{item.kind}</Tag>}
                />
              </List.Item>
            )}
          />
        ) : (
          <Empty
            description={conversationId ? t('common.noData') : t('common.noData')}
            style={{ marginTop: 48 }}
          />
        ),
      },
    ],
    [t, conversationId, contextSources, toolCalls, messages, conversation, conversationArtifacts, agentRuns, agentProfile, latestRun, runTimeline],
  );

  return (
    <div
      style={{
        width: visible ? 360 : 0,
        minWidth: visible ? 360 : 0,
        overflow: 'hidden',
        transition: 'width 0.2s ease, min-width 0.2s ease',
        borderLeft: visible ? '1px solid var(--border-color)' : 'none',
        backgroundColor: token.colorBgContainer,
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      {visible && (
        <Tabs
          activeKey={activeTab}
          onChange={onTabChange}
          items={tabItems}
          size="small"
          style={{ flex: 1, padding: '0 12px' }}
        />
      )}
    </div>
  );
}
