import { useEffect, useMemo, useState } from 'react';
import { App, Button, Card, Empty, Form, Input, List, Modal, Select, Space, Switch, Tag, Typography, theme } from 'antd';
import { Bot, CheckCircle2, PlugZap, Plus, RefreshCw, Save, Trash2 } from 'lucide-react';
import { useExternalAgentStore } from '@/stores';
import type { ExternalAgent } from '@/types';
import { getExternalTaskSummary } from '@/lib/externalTaskSummary';

const DEFAULT_CAPABILITIES = JSON.stringify({ taskKinds: ['general'], resultIngest: ['assistant_message'] }, null, 2);

interface AgentFormValues {
  name: string;
  kind: string;
  baseUrl?: string;
  authType: string;
  authValue?: string;
  capabilitiesJson: string;
  enabled: boolean;
}

function authValueToJson(authType: string, authValue?: string): string | null {
  const value = authValue?.trim();
  if (!value) return null;
  if (authType === 'bearer') return JSON.stringify({ token: value });
  if (authType === 'api_key') return JSON.stringify({ apiKey: value, header: 'X-API-Key' });
  return null;
}

function authValueFromJson(agent?: ExternalAgent | null): string | undefined {
  if (!agent?.authConfigJson) return undefined;
  try {
    const parsed = JSON.parse(agent.authConfigJson) as { token?: string; apiKey?: string; api_key?: string };
    return parsed.token || parsed.apiKey || parsed.api_key;
  } catch {
    return undefined;
  }
}

export default function ExternalAgentSettings() {
  const { token } = theme.useToken();
  const { message, modal } = App.useApp();
  const [form] = Form.useForm<AgentFormValues>();
  const agents = useExternalAgentStore((state) => state.agents);
  const tasks = useExternalAgentStore((state) => state.tasks);
  const loading = useExternalAgentStore((state) => state.loading);
  const loadAgents = useExternalAgentStore((state) => state.loadAgents);
  const loadTasks = useExternalAgentStore((state) => state.loadTasks);
  const createAgent = useExternalAgentStore((state) => state.createAgent);
  const updateAgent = useExternalAgentStore((state) => state.updateAgent);
  const deleteAgent = useExternalAgentStore((state) => state.deleteAgent);
  const testAgent = useExternalAgentStore((state) => state.testAgent);
  const retryTask = useExternalAgentStore((state) => state.retryTask);
  const syncTask = useExternalAgentStore((state) => state.syncTask);
  const listTaskEvents = useExternalAgentStore((state) => state.listTaskEvents);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [taskEventsOpen, setTaskEventsOpen] = useState(false);
  const [taskEvents, setTaskEvents] = useState<Array<{ id: string; eventType: string; payloadJson: string; createdAt: number }>>([]);
  const selectedAgent = useMemo(
    () => agents.find((agent) => agent.id === selectedId) ?? null,
    [agents, selectedId],
  );

  useEffect(() => {
    void loadAgents();
    void loadTasks({ limit: 20 });
  }, [loadAgents, loadTasks]);

  useEffect(() => {
    if (!selectedAgent) {
      form.setFieldsValue({
        name: '',
        kind: 'generic_http',
        baseUrl: '',
        authType: 'none',
        authValue: '',
        capabilitiesJson: DEFAULT_CAPABILITIES,
        enabled: true,
      });
      return;
    }
    form.setFieldsValue({
      name: selectedAgent.name,
      kind: selectedAgent.kind,
      baseUrl: selectedAgent.baseUrl ?? '',
      authType: selectedAgent.authType,
      authValue: authValueFromJson(selectedAgent),
      capabilitiesJson: selectedAgent.capabilitiesJson || DEFAULT_CAPABILITIES,
      enabled: selectedAgent.enabled,
    });
  }, [form, selectedAgent]);

  const saveAgent = async () => {
    const values = await form.validateFields();
    JSON.parse(values.capabilitiesJson || '{}');
    const payload = {
      name: values.name,
      kind: values.kind,
      baseUrl: values.baseUrl?.trim() || null,
      authType: values.authType,
      authConfigJson: authValueToJson(values.authType, values.authValue),
      capabilitiesJson: values.capabilitiesJson || '{}',
      enabled: values.enabled,
    };
    if (selectedAgent) {
      const updated = await updateAgent(selectedAgent.id, payload);
      setSelectedId(updated.id);
      message.success('外部 Agent 已保存');
    } else {
      const created = await createAgent(payload);
      setSelectedId(created.id);
      message.success('外部 Agent 已创建');
    }
    void loadTasks({ limit: 20 });
  };

  const removeAgent = async () => {
    if (!selectedAgent) return;
    modal.confirm({
      title: '删除外部 Agent',
      content: `确定删除 ${selectedAgent.name} 吗？相关任务记录也会删除。`,
      okButtonProps: { danger: true },
      onOk: async () => {
        await deleteAgent(selectedAgent.id);
        setSelectedId(null);
        message.success('已删除');
      },
    });
  };

  const runTest = async () => {
    if (!selectedAgent) return;
    try {
      const result = await testAgent(selectedAgent.id);
      if (result.ok) {
        message.success(`连接可用：${result.message ?? result.status ?? 'OK'}`);
      } else {
        message.error(result.message ?? '连接失败');
      }
    } catch (e) {
      message.error(`连接测试失败：${String(e)}`);
    }
  };
  const openTaskEvents = async (taskId: string) => {
    const events = await listTaskEvents(taskId);
    setSelectedTaskId(taskId);
    setTaskEvents(events);
    setTaskEventsOpen(true);
  };

  const refreshTask = async (taskId: string) => {
    const result = await syncTask(taskId);
    if (result.assistantMessage) {
      message.success('External task result was synced back into the conversation');
    } else {
      message.success(`Task status updated: ${result.task.status}`);
    }
    if (selectedTaskId === taskId && taskEventsOpen) {
      const events = await listTaskEvents(taskId);
      setTaskEvents(events);
    }
  };

  const rerunTask = async (taskId: string) => {
    const result = await retryTask(taskId);
    if (result.assistantMessage) {
      message.success('A new external task was dispatched and synced back to the conversation');
    } else {
      message.success(`Retried as a new task: ${result.task.status}`);
    }
    void loadTasks({ limit: 20 });
    if (selectedTaskId === taskId && taskEventsOpen) {
      const events = await listTaskEvents(taskId);
      setTaskEvents(events);
    }
  };

  return (
    <div className="h-full overflow-y-auto" style={{ padding: 24 }}>
      <div className="mb-5 flex items-center justify-between">
        <div>
          <Typography.Title level={4} style={{ margin: 0 }}>外部 Agent</Typography.Title>
          <Typography.Text type="secondary">把 NanoClaw 或其他 OpenClaw 类服务作为 AQBot 的外部执行器。</Typography.Text>
        </div>
        <Button icon={<RefreshCw size={16} />} onClick={() => { void loadAgents(); void loadTasks({ limit: 20 }); }}>
          刷新
        </Button>
      </div>

      <div className="grid gap-4" style={{ gridTemplateColumns: 'minmax(240px, 320px) minmax(0, 1fr)' }}>
        <Card size="small" title="Agent 列表" extra={<Button size="small" icon={<Plus size={14} />} onClick={() => setSelectedId(null)}>新建</Button>}>
          {agents.length === 0 ? (
            <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无外部 Agent" />
          ) : (
            <List
              dataSource={agents}
              loading={loading}
              renderItem={(agent) => (
                <List.Item
                  className="cursor-pointer"
                  style={{
                    padding: '10px 8px',
                    borderRadius: 6,
                    background: agent.id === selectedId ? token.colorFillSecondary : undefined,
                  }}
                  onClick={() => setSelectedId(agent.id)}
                >
                  <List.Item.Meta
                    avatar={<Bot size={18} />}
                    title={<Space><span>{agent.name}</span>{agent.enabled ? <Tag color="green">启用</Tag> : <Tag>停用</Tag>}</Space>}
                    description={agent.baseUrl || agent.kind}
                  />
                </List.Item>
              )}
            />
          )}
        </Card>

        <Card
          size="small"
          title={selectedAgent ? '编辑 Agent' : '新建 Agent'}
          extra={(
            <Space>
              {selectedAgent && <Button icon={<PlugZap size={16} />} onClick={runTest}>测试</Button>}
              {selectedAgent && <Button danger icon={<Trash2 size={16} />} onClick={removeAgent}>删除</Button>}
              <Button type="primary" icon={<Save size={16} />} onClick={saveAgent}>保存</Button>
            </Space>
          )}
        >
          <Form form={form} layout="vertical" initialValues={{ kind: 'generic_http', authType: 'none', capabilitiesJson: DEFAULT_CAPABILITIES, enabled: true }}>
            <Form.Item name="name" label="名称" rules={[{ required: true, message: '请输入名称' }]}>
              <Input placeholder="NanoClaw 本地服务" />
            </Form.Item>
            <div className="grid gap-3" style={{ gridTemplateColumns: '1fr 1fr' }}>
              <Form.Item name="kind" label="类型">
                <Select
                  options={[
                    { label: '通用 HTTP', value: 'generic_http' },
                    { label: 'NanoClaw', value: 'nanoclaw' },
                    { label: 'OpenClaw compatible', value: 'openclaw' },
                  ]}
                />
              </Form.Item>
              <Form.Item name="enabled" label="启用" valuePropName="checked">
                <Switch />
              </Form.Item>
            </div>
            <Form.Item name="baseUrl" label="服务地址">
              <Input placeholder="http://127.0.0.1:8787" />
            </Form.Item>
            <div className="grid gap-3" style={{ gridTemplateColumns: '180px 1fr' }}>
              <Form.Item name="authType" label="认证方式">
                <Select
                  options={[
                    { label: '无', value: 'none' },
                    { label: 'Bearer Token', value: 'bearer' },
                    { label: 'API Key', value: 'api_key' },
                  ]}
                />
              </Form.Item>
              <Form.Item name="authValue" label="密钥">
                <Input.Password placeholder="可选" />
              </Form.Item>
            </div>
            <Form.Item
              name="capabilitiesJson"
              label="能力声明 JSON"
              rules={[
                {
                  validator: async (_, value) => {
                    JSON.parse(value || '{}');
                  },
                  message: '璇疯緭鍏ュ悎娉?JSON',
                },
              ]}
            >
              <Input.TextArea autoSize={{ minRows: 7, maxRows: 14 }} style={{ fontFamily: 'var(--code-font-family, monospace)' }} />
            </Form.Item>
          </Form>
        </Card>
      </div>

      <Card size="small" title="近期任务" className="mt-4">
        {tasks.length === 0 ? (
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无任务" />
        ) : (
          <List
            dataSource={tasks}
            renderItem={(task) => (
              <List.Item>
                <List.Item.Meta
                  avatar={task.status === 'completed' ? <CheckCircle2 size={18} color={token.colorSuccess} /> : <Bot size={18} />}
                  title={<Space><span>{task.title}</span><Tag>{task.status}</Tag></Space>}
                  description={(
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                      <Typography.Text type="secondary" ellipsis>
                        {getExternalTaskSummary(task)}
                      </Typography.Text>
                      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                        {task.kind}
                      </Typography.Text>
                    </div>
                  )}
                />
                <Space>
                  <Button size="small" onClick={() => { void rerunTask(task.id); }}>
                    Retry
                  </Button>
                  <Button size="small" onClick={() => { void refreshTask(task.id); }}>
                    Sync
                  </Button>
                  <Button size="small" onClick={() => { void openTaskEvents(task.id); }}>
                    Events
                  </Button>
                </Space>
              </List.Item>
            )}
          />
        )}
      </Card>
      <Modal
        title="任务事件"
        open={taskEventsOpen}
        onCancel={() => setTaskEventsOpen(false)}
        footer={null}
        width={760}
      >
        {taskEvents.length === 0 ? (
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无事件" />
        ) : (
          <List
            dataSource={taskEvents}
            renderItem={(event) => (
              <List.Item key={event.id}>
                <List.Item.Meta
                  title={<Space><Tag>{event.eventType}</Tag><Typography.Text type="secondary">把 NanoClaw 或其他 OpenClaw 类服务作为 AQBot 的外部执行器。</Typography.Text></Space>}
                  description={
                    <pre
                      style={{
                        margin: 0,
                        whiteSpace: 'pre-wrap',
                        wordBreak: 'break-word',
                        fontSize: 12,
                        fontFamily: 'var(--code-font-family, monospace)',
                      }}
                    >
                      {event.payloadJson}
                    </pre>
                  }
                />
              </List.Item>
            )}
          />
        )}
        {selectedTaskId && (
          <Typography.Text type="secondary" style={{ display: 'block', marginTop: 12 }}>
            Task ID: {selectedTaskId}
          </Typography.Text>
        )}
      </Modal>
    </div>
  );
}
