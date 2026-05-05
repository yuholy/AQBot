import { useCallback, useRef, useEffect, useState } from 'react';
import { theme } from 'antd';
import { Minus, X, Square } from 'lucide-react';
import { isTauri, invoke } from '@/lib/invoke';
import appLogo from '@/assets/image/logo.png';

const IS_WINDOWS = navigator.userAgent.includes('Windows');
const TITLE_MENU_ITEMS = ['文件', '编辑', '查看', '窗口', '帮助'];

/** Standard Windows "restore down" icon: two overlapping rectangles */
const RestoreIcon = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.2">
    <rect x="3" y="5" width="8" height="7" rx="0.5" />
    <path d="M5 5V3.5a.5.5 0 0 1 .5-.5H12a.5.5 0 0 1 .5.5V10a.5.5 0 0 1-.5.5h-1.5" />
  </svg>
);

export function TitleBar() {
  const { token } = theme.useToken();
  // Windows window controls
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

  // Pre-load Tauri window module for synchronous drag calls
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
      // Delay startDragging slightly so double-click can be detected.
      // If a second mousedown arrives within the threshold,
      // the onDoubleClick handler fires and cancels the pending drag.
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
      {/* Left: App icon + name (Windows only) */}
      {IS_WINDOWS ? (
        <div className="title-bar-nodrag" style={{ display: 'flex', alignItems: 'center', gap: 18, marginRight: 8 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <img src={appLogo} alt="AQBot" style={{ width: 18, height: 18 }} draggable={false} />
            <span style={{ fontSize: 13, fontWeight: 600, color: token.colorTextBase, userSelect: 'none' }}>AQBot</span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 18 }}>
            {TITLE_MENU_ITEMS.map((item) => (
              <span
                key={item}
                style={{
                  color: token.colorTextSecondary,
                  fontSize: 13,
                  lineHeight: '36px',
                  userSelect: 'none',
                }}
              >
                {item}
              </span>
            ))}
          </div>
        </div>
      ) : <div />}

      <div style={{ display: 'flex', alignItems: 'center', gap: 0 }}>

      {/* Windows window controls */}
      {IS_WINDOWS && isTauri() && (
        <div className="title-bar-nodrag" style={{ display: 'flex', alignItems: 'center', marginLeft: 4 }}>
          {/* Minimize */}
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
          {/* Maximize / Restore */}
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
          {/* Close */}
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
