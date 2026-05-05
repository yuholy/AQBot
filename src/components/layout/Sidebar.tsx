import { Tooltip, theme } from 'antd';
import { MessageSquare, BookOpen, Brain, FolderOpen, Sparkles } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useUIStore, useSettingsStore } from '@/stores';
import { getShortcutBinding, formatShortcutForDisplay } from '@/lib/shortcuts';
import type { ShortcutAction } from '@/lib/shortcuts';
import { SidebarUserMenu } from './SidebarUserMenu';
import type { PageKey } from '@/types';

const mainNavItems: { key: PageKey; icon: React.ReactNode; labelKey: string }[] = [
  { key: 'chat', icon: <MessageSquare size={18} />, labelKey: 'nav.chat' },
  { key: 'skills', icon: <Sparkles size={18} />, labelKey: 'nav.skills' },
  { key: 'knowledge', icon: <BookOpen size={18} />, labelKey: 'nav.knowledge' },
  { key: 'memory', icon: <Brain size={18} />, labelKey: 'nav.memory' },
  // Gateway module hidden for now
  // { key: 'gateway', icon: <Router size={18} />, labelKey: 'nav.gateway' },
  { key: 'files', icon: <FolderOpen size={18} />, labelKey: 'nav.files' },
];

export function Sidebar() {
  const { t } = useTranslation();
  const { token } = theme.useToken();
  const activePage = useUIStore((s) => s.activePage);
  const setActivePage = useUIStore((s) => s.setActivePage);
  const settings = useSettingsStore((s) => s.settings);

  const NAV_SHORTCUT_MAP: Partial<Record<PageKey, ShortcutAction>> = {
    // Gateway module hidden for now
    // gateway: 'toggleGateway',
  };

  const renderNavButton = (item: { key: PageKey; icon: React.ReactNode; labelKey: string }) => {
    const isActive = activePage === item.key;
    const label = t(item.labelKey);
    const action = NAV_SHORTCUT_MAP[item.key];
    const title = action
      ? `${label} (${formatShortcutForDisplay(getShortcutBinding(settings, action))})`
      : label;
    return (
      <Tooltip key={item.key} title={title} placement="right">
        <button
          onClick={() => setActivePage(item.key)}
          className="flex items-center justify-center text-base transition-all"
          style={{
            width: 34,
            height: 34,
            borderRadius: 10,
            backgroundColor: isActive ? token.colorPrimaryBg : 'transparent',
            color: isActive ? token.colorPrimary : token.colorTextSecondary,
            boxShadow: isActive ? `inset 0 0 0 1px ${token.colorPrimaryBorder}` : 'none',
          }}
          onMouseEnter={(e) => {
            if (!isActive) {
              e.currentTarget.style.backgroundColor = token.colorFillSecondary;
              e.currentTarget.style.color = token.colorTextBase;
              e.currentTarget.style.boxShadow = 'none';
            }
          }}
          onMouseLeave={(e) => {
            if (!isActive) {
              e.currentTarget.style.backgroundColor = 'transparent';
              e.currentTarget.style.color = token.colorTextSecondary;
              e.currentTarget.style.boxShadow = 'none';
            }
          }}
        >
          {item.icon}
        </button>
      </Tooltip>
    );
  };

  return (
    <div
      className="flex flex-col items-center h-full"
      style={{
        paddingTop: 8,
        paddingBottom: 12,
        paddingLeft: 8,
        paddingRight: 8,
      }}
    >
      <nav className="flex flex-col gap-1.5">
        {mainNavItems.map(renderNavButton)}
      </nav>

      <div className="flex-1" />

      <SidebarUserMenu />
    </div>
  );
}
