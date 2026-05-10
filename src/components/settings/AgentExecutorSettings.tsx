import { Card, List, Space, Tag, Typography } from 'antd';
import { Bot, FolderOpen, ShieldCheck } from 'lucide-react';
import { AGENT_EXECUTORS } from '@/lib/agentExecutors';

function ExecutorIcon() {
  return <Bot size={18} />;
}

function commandHint(): string {
  return 'AQBot built-in agent runtime';
}

export default function AgentExecutorSettings() {
  return (
    <div className="h-full overflow-y-auto" style={{ padding: 24 }}>
      <div className="mb-5">
        <Typography.Title level={4} style={{ margin: 0 }}>Agent 执行器</Typography.Title>
        <Typography.Text type="secondary">
          AQBot 现在仅保留内置本地 Agent 运行时，工作目录与权限模式仍可在对话页配置。
        </Typography.Text>
      </div>

      <Card size="small" title="本地执行器">
        <List
          dataSource={AGENT_EXECUTORS}
          renderItem={(executor) => (
            <List.Item>
              <List.Item.Meta
                avatar={<ExecutorIcon />}
                title={(
                  <Space wrap>
                    <span>{executor.name}</span>
                    <Tag color="blue">Local</Tag>
                  </Space>
                )}
                description={(
                  <Space direction="vertical" size={6}>
                    <Typography.Text type="secondary">{executor.description}</Typography.Text>
                    <Typography.Text code>{commandHint()}</Typography.Text>
                    <Space wrap>
                      {executor.supportsCwd && <Tag icon={<FolderOpen size={12} />}>工作目录</Tag>}
                      {executor.supportsPermissionMode && <Tag icon={<ShieldCheck size={12} />}>权限模式</Tag>}
                    </Space>
                  </Space>
                )}
              />
            </List.Item>
          )}
        />
      </Card>
    </div>
  );
}
