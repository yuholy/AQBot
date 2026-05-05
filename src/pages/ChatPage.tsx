import { useEffect } from 'react';
import { Button, Tooltip, theme } from 'antd';
import { PanelLeftOpen } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useConversationStore, useProviderStore, useUIStore } from '@/stores';
import { ChatSidebar } from '@/components/chat/ChatSidebar';
import { ChatView } from '@/components/chat/ChatView';

export function ChatPage() {
  const { t } = useTranslation();
  const { token } = theme.useToken();
  const fetchConversations = useConversationStore((s) => s.fetchConversations);
  const conversationCount = useConversationStore((s) => s.conversations.length);
  const fetchProviders = useProviderStore((s) => s.fetchProviders);
  const providerCount = useProviderStore((s) => s.providers.length);
  const sidebarCollapsed = useUIStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useUIStore((s) => s.toggleSidebar);

  useEffect(() => {
    if (conversationCount === 0) {
      fetchConversations();
    }
    if (providerCount === 0) {
      fetchProviders();
    }
  }, [conversationCount, fetchConversations, fetchProviders, providerCount]);

  return (
    <div
      className="flex h-full"
      style={{
        overflow: 'hidden',
        gap: 0,
        padding: 0,
        backgroundColor: token.colorBgContainer,
      }}
    >
      <div
        className="h-full"
        style={{
          width: sidebarCollapsed ? 48 : 252,
          minWidth: sidebarCollapsed ? 48 : 252,
          overflow: 'hidden',
          backgroundColor: token.colorFillQuaternary,
          borderRight: `1px solid ${token.colorBorderSecondary}`,
          transition: 'width 0.18s ease, min-width 0.18s ease',
        }}
      >
        {sidebarCollapsed ? (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              paddingTop: 10,
            }}
          >
            <Tooltip title={t('common.expand')} placement="right">
              <Button
                type="text"
                size="small"
                icon={<PanelLeftOpen size={16} />}
                onClick={toggleSidebar}
                aria-label={t('common.expand')}
              />
            </Tooltip>
          </div>
        ) : (
          <ChatSidebar />
        )}
      </div>
      <div
        style={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
          backgroundColor: token.colorBgContainer,
          borderTopLeftRadius: 12,
        }}
      >
        <ChatView />
      </div>
    </div>
  );
}
