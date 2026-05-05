import { Card, List, Space, Tag, Typography } from 'antd';
import { Bot, Brain, Code, FolderOpen, ShieldCheck, Zap } from 'lucide-react';
import { AGENT_EXECUTORS } from '@/lib/agentExecutors';

function ExecutorIcon({ id }: { id: string }) {
  if (id === 'claude-code') return <Code size={18} />;
  if (id === 'deepseek-tui') return <Brain size={18} />;
  return <Bot size={18} />;
}

function commandHint(id: string): string {
  if (id === 'claude-code') return 'claude -p <prompt> --output-format text';
  if (id === 'deepseek-tui') return 'deepseek-tui exec <prompt>';
  return 'AQBot built-in agent runtime';
}

export default function AgentExecutorSettings() {
  return (
    <div className="h-full overflow-y-auto" style={{ padding: 24 }}>
      <div className="mb-5">
        <Typography.Title level={4} style={{ margin: 0 }}>Agent 执行器</Typography.Title>
        <Typography.Text type="secondary">
          管理本地 Agent 执行入口。对话页只选择执行器，具体能力和命令约定在这里沉淀。
        </Typography.Text>
      </div>

      <Card size="small" title="本地执行器">
        <List
          dataSource={AGENT_EXECUTORS}
          renderItem={(executor) => (
            <List.Item>
              <List.Item.Meta
                avatar={<ExecutorIcon id={executor.id} />}
                title={
                  <Space wrap>
                    <span>{executor.name}</span>
                    <Tag color="blue">Local</Tag>
                    {executor.supportsAutoMode && <Tag color="processing">Auto</Tag>}
                  </Space>
                }
                description={
                  <Space direction="vertical" size={6}>
                    <Typography.Text type="secondary">{executor.description}</Typography.Text>
                    <Typography.Text code>{commandHint(executor.id)}</Typography.Text>
                    <Space wrap>
                      {executor.supportsCwd && <Tag icon={<FolderOpen size={12} />}>工作目录</Tag>}
                      {executor.supportsPermissionMode && <Tag icon={<ShieldCheck size={12} />}>权限模式</Tag>}
                      {executor.supportsAutoMode && <Tag icon={<Zap size={12} />}>自动执行</Tag>}
                    </Space>
                  </Space>
                }
              />
            </List.Item>
          )}
        />
      </Card>
    </div>
  );
}
