import { useCallback, useEffect, useRef, useState } from 'react';
import { theme } from 'antd';
import { Minus, PanelTop, Square, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { isTauri, invoke } from '@/lib/invoke';
import { useUIStore } from '@/stores';
import appLogo from '@/assets/image/logo.png';

const IS_WINDOWS = navigator.userAgent.includes('Windows');

const RestoreIcon = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.2">
    <rect x="3" y="5" width="8" height="7" rx="0.5" />
    <path d="M5 5V3.5a.5.5 0 0 1 .5-.5H12a.5.5 0 0 1 .5.5V10a.5.5 0 0 1-.5.5h-1.5" />
  </svg>
);

export function TitleBar() {
  const { t } = useTranslation();
  const { token } = theme.useToken();
  const activePage = useUIStore((s) => s.activePage);
  const sidebarCollapsed = useUIStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useUIStore((s) => s.toggleSidebar);
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    if (!IS_WINDOWS || !isTauri()) return;
    let unlisten: (() => void) | undefined;
    (async () => {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const win = getCurrentWindow();
      setIsMaximized(await win.isMaximized());
      unlisten = await win.onResized(async () => {
        setIsMaximized(await win.isMaximized());
      });
    })();
    return () => { unlisten?.(); };
  }, []);

  const handleWindowMinimize = useCallback(async () => {
    await invoke('minimize_window');
  }, []);

  const handleWindowMaximize = useCallback(async () => {
    await invoke('toggle_maximize_window');
  }, []);

  const handleWindowClose = useCallback(async () => {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().close();
  }, []);

  const tauriWindowRef = useRef<typeof import('@tauri-apps/api/window') | null>(null);
  useEffect(() => {
    if (isTauri()) {
      import('@tauri-apps/api/window').then((mod) => {
        tauriWindowRef.current = mod;
      });
    }
  }, []);

  const dragTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleDragMouseDown = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (target.closest('button')) return;
    const mod = tauriWindowRef.current;
    if (!mod) return;
    e.preventDefault();

    if (IS_WINDOWS) {
      if (dragTimerRef.current) clearTimeout(dragTimerRef.current);
      dragTimerRef.current = setTimeout(() => {
        mod.getCurrentWindow().startDragging();
      }, 200);
    } else {
      mod.getCurrentWindow().startDragging();
    }
  }, []);

  const handleTitleBarDoubleClick = useCallback(() => {
    if (!IS_WINDOWS) return;
    if (dragTimerRef.current) {
      clearTimeout(dragTimerRef.current);
      dragTimerRef.current = null;
    }
    invoke('toggle_maximize_window');
  }, []);

  return (
    <div
      className="title-bar-drag"
      {...(!IS_WINDOWS ? { 'data-tauri-drag-region': true } : {})}
      onMouseDown={handleDragMouseDown}
      onDoubleClick={IS_WINDOWS ? handleTitleBarDoubleClick : undefined}
      style={{
        height: 36,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        paddingLeft: IS_WINDOWS ? 12 : 72,
        paddingRight: IS_WINDOWS ? 0 : 12,
        backgroundColor: token.colorBgContainer,
        borderBottom: `1px solid ${token.colorBorderSecondary}`,
        flexShrink: 0,
      }}
    >
      {IS_WINDOWS ? (
        <div className="title-bar-nodrag" style={{ display: 'flex', alignItems: 'center', gap: 0, marginRight: 8 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 80 }}>
            <img src={appLogo} alt="AQBot" style={{ width: 18, height: 18 }} draggable={false} />
            <span style={{ fontSize: 13, fontWeight: 600, color: token.colorTextBase, userSelect: 'none' }}>AQBot</span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <div
              style={{
                width: 30,
                height: 28,
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                flexShrink: 0,
              }}
            >
              {activePage === 'chat' ? (
                <button
                  type="button"
                  onClick={toggleSidebar}
                  aria-label={sidebarCollapsed ? t('common.expand') : t('common.collapse')}
                  style={{
                    width: 30,
                    height: 28,
                    display: 'inline-flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    padding: 0,
                    border: `1px solid ${token.colorBorderSecondary}`,
                    borderRadius: 8,
                    background: token.colorBgContainer,
                    color: token.colorTextSecondary,
                    cursor: 'pointer',
                    userSelect: 'none',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.backgroundColor = token.colorFillTertiary;
                    e.currentTarget.style.color = token.colorTextBase;
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.backgroundColor = token.colorBgContainer;
                    e.currentTarget.style.color = token.colorTextSecondary;
                  }}
                >
                  <PanelTop size={14} style={{ transform: sidebarCollapsed ? 'rotate(90deg)' : 'rotate(-90deg)' }} />
                </button>
              ) : (
                <span
                  aria-hidden="true"
                  style={{
                    width: 30,
                    height: 28,
                    display: 'inline-flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    opacity: 0.18,
                    color: token.colorTextQuaternary,
                    pointerEvents: 'none',
                  }}
                >
                  <PanelTop size={14} style={{ transform: 'rotate(90deg)' }} />
                </span>
              )}
            </div>
          </div>
        </div>
      ) : <div />}

      <div style={{ display: 'flex', alignItems: 'center', gap: 0 }}>
        {IS_WINDOWS && isTauri() && (
          <div className="title-bar-nodrag" style={{ display: 'flex', alignItems: 'center', marginLeft: 4 }}>
            <button
              onClick={handleWindowMinimize}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                width: 46,
                height: 36,
                border: 'none',
                background: 'transparent',
                color: token.colorTextSecondary,
                cursor: 'pointer',
                outline: 'none',
              }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = token.colorFillSecondary; e.currentTarget.style.color = token.colorTextBase; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = token.colorTextSecondary; }}
            >
              <Minus size={16} />
            </button>
            <button
              onClick={handleWindowMaximize}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                width: 46,
                height: 36,
                border: 'none',
                background: 'transparent',
                color: token.colorTextSecondary,
                cursor: 'pointer',
                outline: 'none',
              }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = token.colorFillSecondary; e.currentTarget.style.color = token.colorTextBase; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = token.colorTextSecondary; }}
            >
              {isMaximized ? <RestoreIcon /> : <Square size={14} />}
            </button>
            <button
              onClick={handleWindowClose}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                width: 46,
                height: 36,
                border: 'none',
                background: 'transparent',
                color: token.colorTextSecondary,
                cursor: 'pointer',
                outline: 'none',
                borderRadius: 0,
              }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = '#e81123'; e.currentTarget.style.color = '#ffffff'; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = token.colorTextSecondary; }}
            >
              <X size={16} />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
