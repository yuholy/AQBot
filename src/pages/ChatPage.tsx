import { useEffect } from 'react';
import { theme } from 'antd';
import { useConversationStore, useProviderStore, useUIStore } from '@/stores';
import { ChatSidebar } from '@/components/chat/ChatSidebar';
import { ChatView } from '@/components/chat/ChatView';

export function ChatPage() {
  const { token } = theme.useToken();
  const fetchConversations = useConversationStore((s) => s.fetchConversations);
  const conversationCount = useConversationStore((s) => s.conversations.length);
  const fetchProviders = useProviderStore((s) => s.fetchProviders);
  const providerCount = useProviderStore((s) => s.providers.length);
  const sidebarCollapsed = useUIStore((s) => s.sidebarCollapsed);

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
        position: 'relative',
        overflow: 'hidden',
        gap: 0,
        padding: 0,
        backgroundColor: token.colorBgContainer,
      }}
    >
      <div
        className="h-full"
        style={{
          width: sidebarCollapsed ? 0 : 252,
          minWidth: sidebarCollapsed ? 0 : 252,
          overflow: 'hidden',
          backgroundColor: token.colorFillQuaternary,
          borderRight: sidebarCollapsed ? 'none' : `1px solid ${token.colorBorderSecondary}`,
          transition: 'width 0.18s ease, min-width 0.18s ease, border-color 0.18s ease',
        }}
      >
        {!sidebarCollapsed && <ChatSidebar />}
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
