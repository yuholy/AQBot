import { useCallback, useEffect, useMemo, useState } from 'react';
import { App, Avatar, Button, Divider, Dropdown, Popover, Space, Spin, Tooltip, Typography, theme } from 'antd';
import type { MenuProps } from 'antd';
import { Bug, CloudUpload, Github, Globe, MessageSquarePlus, Monitor, Moon, Pin, PinOff, RotateCcw, Settings, Star, Sun, User } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useResolvedAvatarSrc } from '@/hooks/useResolvedAvatarSrc';
import { LANG_OPTIONS } from '@/lib/constants';
import { getShortcutBinding, formatShortcutForDisplay } from '@/lib/shortcuts';
import { invoke, isTauri } from '@/lib/invoke';
import { useUIStore, useSettingsStore } from '@/stores';
import { useBackupStore } from '@/stores/backupStore';
import { useUserProfileStore } from '@/stores/userProfileStore';
import { UserProfileModal } from './UserProfileModal';

const THEME_OPTIONS = [
  { key: 'system', icon: <Monitor size={14} />, labelKey: 'settings.themeSystem' },
  { key: 'light', icon: <Sun size={14} />, labelKey: 'settings.themeLight' },
  { key: 'dark', icon: <Moon size={14} />, labelKey: 'settings.themeDark' },
] as const;

const THEME_ICONS: Record<string, React.ReactNode> = {
  system: <Monitor size={14} />,
  light: <Sun size={14} />,
  dark: <Moon size={14} />,
};

export function SidebarUserMenu() {
  const { t, i18n } = useTranslation();
  const { token } = theme.useToken();
  const { modal, message } = App.useApp();
  const profile = useUserProfileStore((s) => s.profile);
  const loadProfile = useUserProfileStore((s) => s.loadProfile);
  const resolvedAvatarSrc = useResolvedAvatarSrc(profile.avatarType, profile.avatarValue);
  const activePage = useUIStore((s) => s.activePage);
  const enterSettings = useUIStore((s) => s.enterSettings);
  const exitSettings = useUIStore((s) => s.exitSettings);
  const themeMode = useSettingsStore((s) => s.settings.theme_mode);
  const alwaysOnTop = useSettingsStore((s) => s.settings.always_on_top);
  const saveSettings = useSettingsStore((s) => s.saveSettings);
  const settings = useSettingsStore((s) => s.settings);
  const isInSettings = activePage === 'settings';
  const [menuOpen, setMenuOpen] = useState(false);
  const [profileModalOpen, setProfileModalOpen] = useState(false);
  const [backupPopoverOpen, setBackupPopoverOpen] = useState(false);
  const [backingUp, setBackingUp] = useState<'local' | 'webdav' | null>(null);
  const [pinned, setPinned] = useState(alwaysOnTop ?? false);
  const [lastLocalBackup, setLastLocalBackup] = useState<string | null>(null);
  const [lastWebDavSync, setLastWebDavSync] = useState<string | null>(null);
  const [nextLocalTs, setNextLocalTs] = useState<number | null>(null);
  const [nextWebDavTs, setNextWebDavTs] = useState<number | null>(null);
  const [countdownText, setCountdownText] = useState<string | null>(null);
  const [popoverLocalCountdown, setPopoverLocalCountdown] = useState<string | null>(null);
  const [popoverWebDavCountdown, setPopoverWebDavCountdown] = useState<string | null>(null);
  const { backupSettings, loadBackupSettings } = useBackupStore();

  useEffect(() => {
    loadProfile().catch(() => {});
  }, [loadProfile]);

  useEffect(() => {
    setPinned(alwaysOnTop ?? false);
  }, [alwaysOnTop]);

  const themeMenuItems: MenuProps['items'] = THEME_OPTIONS.map((opt) => ({
    key: opt.key,
    icon: opt.icon,
    label: t(opt.labelKey),
  }));

  const langMenuItems: MenuProps['items'] = LANG_OPTIONS.map((opt) => ({
    key: opt.key,
    icon: <span>{opt.icon}</span>,
    label: opt.label,
  }));

  const GITHUB_REPO = 'https://github.com/AQBot-Desktop/AQBot';
  const githubMenuItems: MenuProps['items'] = [
    { key: 'feature', icon: <MessageSquarePlus size={14} />, label: t('titlebar.submitFeature') },
    { key: 'bug', icon: <Bug size={14} />, label: t('titlebar.submitBug') },
    { type: 'divider' },
    { key: 'star', icon: <Star size={14} />, label: t('titlebar.giveStar') },
  ];

  const fmtCountdown = useCallback((ms: number) => {
    if (ms <= 0) return t('titlebar.now');
    const h = Math.floor(ms / 3600000);
    const m = Math.floor((ms % 3600000) / 60000);
    const s = Math.floor((ms % 60000) / 1000);
    if (h > 0) return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    return `${m}:${s.toString().padStart(2, '0')}`;
  }, [t]);

  useEffect(() => {
    loadBackupSettings();

    invoke<{ lastSyncTime: string | null }>('get_webdav_sync_status')
      .then((s) => {
        if (s.lastSyncTime) {
          const d = new Date(s.lastSyncTime);
          if (!Number.isNaN(d.getTime())) setLastWebDavSync(d.toLocaleString());
        }
      })
      .catch(() => {});

    invoke<Array<{ createdAt: string }>>('list_backups')
      .then((list) => {
        if (list.length > 0) {
          const raw = list[0].createdAt;
          const d = new Date(raw.includes('T') || raw.includes('Z') ? raw : `${raw}Z`);
          if (!Number.isNaN(d.getTime())) setLastLocalBackup(d.toLocaleString());
        }
      })
      .catch(() => {});
  }, [backupPopoverOpen, loadBackupSettings]);

  useEffect(() => {
    if (!lastWebDavSync) {
      setNextWebDavTs(null);
      return;
    }
    const d = new Date(lastWebDavSync);
    if (Number.isNaN(d.getTime())) return;
    const interval = settings.webdav_sync_interval_minutes ?? 60;
    if (settings.webdav_sync_enabled && interval > 0) {
      const intervalMs = interval * 60000;
      let next = d.getTime() + intervalMs;
      while (next < Date.now()) next += intervalMs;
      setNextWebDavTs(next);
    } else {
      setNextWebDavTs(null);
    }
  }, [settings.webdav_sync_enabled, settings.webdav_sync_interval_minutes, lastWebDavSync]);

  useEffect(() => {
    if (!backupSettings?.enabled) {
      setNextLocalTs(null);
      return;
    }
    const intervalMs = (backupSettings.intervalHours ?? 24) * 3600000;
    if (lastLocalBackup) {
      const lastTime = new Date(lastLocalBackup).getTime();
      if (!Number.isNaN(lastTime)) {
        let next = lastTime + intervalMs;
        while (next < Date.now()) next += intervalMs;
        setNextLocalTs(next);
        return;
      }
    }
    setNextLocalTs(Date.now() + intervalMs);
  }, [backupSettings, lastLocalBackup]);

  useEffect(() => {
    const tick = () => {
      const now = Date.now();
      let soonest: number | null = null;
      if (nextLocalTs) soonest = nextLocalTs;
      if (nextWebDavTs && (!soonest || nextWebDavTs < soonest)) soonest = nextWebDavTs;
      setCountdownText(soonest ? fmtCountdown(soonest - now) : null);
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [fmtCountdown, nextLocalTs, nextWebDavTs]);

  useEffect(() => {
    if (!backupPopoverOpen) return;
    const tick = () => {
      const now = Date.now();
      setPopoverLocalCountdown(nextLocalTs && nextLocalTs > now ? `${new Date(nextLocalTs).toLocaleString()} (${fmtCountdown(nextLocalTs - now)})` : null);
      setPopoverWebDavCountdown(nextWebDavTs && nextWebDavTs > now ? `${new Date(nextWebDavTs).toLocaleString()} (${fmtCountdown(nextWebDavTs - now)})` : null);
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [backupPopoverOpen, fmtCountdown, nextLocalTs, nextWebDavTs]);

  const handlePinToggle = useCallback(async () => {
    const next = !pinned;
    setPinned(next);
    try {
      await invoke('set_always_on_top', { enabled: next });
      saveSettings({ always_on_top: next });
    } catch {
      setPinned(!next);
    }
  }, [pinned, saveSettings]);

  const handleThemeChange: MenuProps['onClick'] = useCallback(({ key }: { key: string }) => {
    saveSettings({ theme_mode: key });
  }, [saveSettings]);

  const handleLangChange: MenuProps['onClick'] = useCallback(({ key }: { key: string }) => {
    i18n.changeLanguage(key);
    saveSettings({ language: key });
  }, [i18n, saveSettings]);

  const handleSettingsToggle = useCallback(() => {
    if (isInSettings) exitSettings();
    else enterSettings();
    setMenuOpen(false);
  }, [enterSettings, exitSettings, isInSettings]);

  const handleReload = useCallback(() => {
    modal.confirm({
      title: t('desktop.reloadConfirmTitle'),
      content: t('desktop.reloadConfirmContent'),
      okText: t('desktop.reloadConfirmOk'),
      cancelText: t('desktop.reloadConfirmCancel'),
      onOk: () => window.location.reload(),
    });
  }, [modal, t]);

  const handleQuickBackup = useCallback(async (type: 'local' | 'webdav') => {
    setBackingUp(type);
    try {
      if (type === 'local') await invoke('create_backup', { format: 'zip' });
      else await invoke('webdav_backup');
      message.success(t('backup.backupSuccess'));
      setBackupPopoverOpen(false);
    } catch (e) {
      message.error(String(e));
    } finally {
      setBackingUp(null);
    }
  }, [message, t]);

  const handleGithubClick: MenuProps['onClick'] = useCallback(({ key }: { key: string }) => {
    let url = GITHUB_REPO;
    if (key === 'feature') url = `${GITHUB_REPO}/issues/new?labels=enhancement&template=feature_request.yml`;
    else if (key === 'bug') url = `${GITHUB_REPO}/issues/new?labels=bug&template=bug_report.yml`;
    if (isTauri()) {
      import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl(url)).catch(() => window.open(url, '_blank'));
    } else {
      window.open(url, '_blank');
    }
  }, []);

  const triggerButtonStyle: React.CSSProperties = useMemo(() => ({
    width: 34,
    height: 34,
    padding: 0,
    border: 'none',
    background: 'none',
    borderRadius: 14,
  }), []);

  const quickActionButtonStyle: React.CSSProperties = useMemo(() => ({
    width: '100%',
    height: 54,
    padding: '8px 10px',
    borderRadius: 12,
    color: token.colorTextSecondary,
    backgroundColor: token.colorFillTertiary,
    border: `1px solid ${token.colorBorderSecondary}`,
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'flex-start',
    justifyContent: 'center',
    gap: 6,
    textAlign: 'left',
  }), [token.colorBorderSecondary, token.colorFillTertiary, token.colorTextSecondary]);

  const actionLabelStyle: React.CSSProperties = useMemo(() => ({
    fontSize: 12,
    lineHeight: 1.1,
    fontWeight: 500,
  }), []);

  const actionMetaStyle: React.CSSProperties = useMemo(() => ({
    fontSize: 11,
    lineHeight: 1.1,
    color: token.colorTextTertiary,
  }), [token.colorTextTertiary]);

  const renderUserAvatar = () => {
    const size = 34;
    if (profile.avatarType === 'emoji' && profile.avatarValue) {
      return (
        <div
          style={{
            width: size,
            height: size,
            borderRadius: 14,
            backgroundColor: token.colorFillQuaternary,
            boxShadow: `inset 0 0 0 1px ${token.colorBorderSecondary}`,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: 16,
            cursor: 'pointer',
          }}
        >
          {profile.avatarValue}
        </div>
      );
    }
    if ((profile.avatarType === 'url' || profile.avatarType === 'file') && profile.avatarValue) {
      const src = profile.avatarType === 'file' ? resolvedAvatarSrc : profile.avatarValue;
      return <Avatar size={size} src={src} style={{ cursor: 'pointer', borderRadius: 14, boxShadow: `inset 0 0 0 1px ${token.colorBorderSecondary}` }} />;
    }
    return (
      <Avatar
        size={size}
        icon={<User size={16} />}
        style={{ cursor: 'pointer', backgroundColor: token.colorPrimary, boxShadow: `inset 0 0 0 1px ${token.colorPrimaryBorder}` }}
      />
    );
  };

  const backupContent = (
    <div style={{ width: 240 }}>
      <Typography.Text strong style={{ fontSize: 13 }}>
        {t('titlebar.lastBackup')}
      </Typography.Text>
      <Space direction="vertical" size={2} style={{ width: '100%', marginTop: 4 }}>
        {lastLocalBackup && <Typography.Text type="secondary" style={{ fontSize: 12 }}>{t('titlebar.lastLocal')}: {lastLocalBackup}</Typography.Text>}
        {lastWebDavSync && <Typography.Text type="secondary" style={{ fontSize: 12 }}>WebDAV: {lastWebDavSync}</Typography.Text>}
        {!lastLocalBackup && !lastWebDavSync && <Typography.Text type="secondary" style={{ fontSize: 12 }}>{t('titlebar.noBackupYet')}</Typography.Text>}
      </Space>
      {(popoverLocalCountdown || popoverWebDavCountdown) && (
        <>
          <Divider style={{ margin: '6px 0' }} />
          <Typography.Text strong style={{ fontSize: 13 }}>{t('titlebar.nextBackup')}</Typography.Text>
          <Space direction="vertical" size={2} style={{ width: '100%', marginTop: 4 }}>
            {popoverLocalCountdown && <Typography.Text type="secondary" style={{ fontSize: 12 }}>{t('titlebar.lastLocal')}: {popoverLocalCountdown}</Typography.Text>}
            {popoverWebDavCountdown && <Typography.Text type="secondary" style={{ fontSize: 12 }}>WebDAV: {popoverWebDavCountdown}</Typography.Text>}
          </Space>
        </>
      )}
      <Divider style={{ margin: '6px 0' }} />
      <Space direction="vertical" size={8} style={{ width: '100%' }}>
        <button
          onClick={() => handleQuickBackup('local')}
          disabled={backingUp !== null}
          style={{
            width: '100%',
            padding: '6px 10px',
            borderRadius: token.borderRadius,
            border: `1px solid ${token.colorBorder}`,
            backgroundColor: 'transparent',
            cursor: backingUp ? 'not-allowed' : 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: 6,
            color: token.colorText,
          }}
        >
          {backingUp === 'local' ? <Spin size="small" /> : <CloudUpload size={14} />}
          {t('titlebar.localBackup')}
        </button>
        <button
          onClick={() => handleQuickBackup('webdav')}
          disabled={backingUp !== null}
          style={{
            width: '100%',
            padding: '6px 10px',
            borderRadius: token.borderRadius,
            border: `1px solid ${token.colorBorder}`,
            backgroundColor: 'transparent',
            cursor: backingUp ? 'not-allowed' : 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: 6,
            color: token.colorText,
          }}
        >
          {backingUp === 'webdav' ? <Spin size="small" /> : <CloudUpload size={14} />}
          {t('titlebar.webdavBackup')}
        </button>
      </Space>
    </div>
  );

  const popoverContent = (
    <div style={{ width: 292, display: 'grid', gap: 12 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        {renderUserAvatar()}
        <div style={{ minWidth: 0, flex: 1 }}>
          <Typography.Text strong style={{ display: 'block' }}>
            {profile.name || 'AQBot User'}
          </Typography.Text>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {themeMode === 'dark' ? t('settings.themeDark') : themeMode === 'light' ? t('settings.themeLight') : t('settings.themeSystem')}
            {' / '}
            {LANG_OPTIONS.find((opt) => opt.key === i18n.language)?.label ?? i18n.language}
          </Typography.Text>
          {countdownText ? (
            <Typography.Text style={{ display: 'block', fontSize: 11, color: token.colorPrimary, marginTop: 4 }}>
              {t('titlebar.nextBackup')}: {countdownText}
            </Typography.Text>
          ) : null}
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
        <Button
          type="text"
          onClick={() => {
            setProfileModalOpen(true);
            setMenuOpen(false);
          }}
          style={{
            ...quickActionButtonStyle,
            color: token.colorText,
          }}
        >
          <User size={14} />
          <span style={actionLabelStyle}>{t('userProfile.title')}</span>
          <span style={actionMetaStyle}>{profile.name || 'AQBot User'}</span>
        </Button>

        <Button
          type="text"
          onClick={handleSettingsToggle}
          style={{
            ...quickActionButtonStyle,
            color: isInSettings ? token.colorError : token.colorText,
          }}
        >
          <Settings size={14} />
          <span style={actionLabelStyle}>{isInSettings ? t('settings.closeSettings') : t('settings.openSettings')}</span>
          <span style={actionMetaStyle}>{formatShortcutForDisplay(getShortcutBinding(settings, 'openSettings'))}</span>
        </Button>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 8 }}>
        <Tooltip title={t('desktop.alwaysOnTop')}>
          <Button
            type="text"
            style={{
              ...quickActionButtonStyle,
              color: pinned ? token.colorPrimary : token.colorTextSecondary,
            }}
            onClick={handlePinToggle}
          >
            {pinned ? <Pin size={14} /> : <PinOff size={14} />}
            <span style={actionLabelStyle}>{t('desktop.alwaysOnTop')}</span>
            <span style={actionMetaStyle}>{pinned ? 'ON' : 'OFF'}</span>
          </Button>
        </Tooltip>

        <Dropdown menu={{ items: themeMenuItems, onClick: handleThemeChange, selectedKeys: [themeMode] }} trigger={['click']} placement="topLeft" destroyOnHidden>
          <Button type="text" style={quickActionButtonStyle}>
            {THEME_ICONS[themeMode] ?? <Monitor size={14} />}
            <span style={actionLabelStyle}>{t('settings.theme')}</span>
            <span style={actionMetaStyle}>
              {themeMode === 'dark' ? t('settings.themeDark') : themeMode === 'light' ? t('settings.themeLight') : t('settings.themeSystem')}
            </span>
          </Button>
        </Dropdown>

        <Dropdown menu={{ items: langMenuItems, onClick: handleLangChange, selectedKeys: [i18n.language] }} trigger={['click']} placement="topLeft" destroyOnHidden>
          <Button type="text" style={quickActionButtonStyle}>
            <Globe size={14} />
            <span style={actionLabelStyle}>{t('settings.language')}</span>
            <span style={actionMetaStyle}>{LANG_OPTIONS.find((opt) => opt.key === i18n.language)?.label ?? i18n.language}</span>
          </Button>
        </Dropdown>
      </div>

      <Divider style={{ margin: 0 }} />

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 8 }}>
        <Popover open={backupPopoverOpen} onOpenChange={setBackupPopoverOpen} trigger="click" placement="topLeft" destroyTooltipOnHide content={backupContent}>
          <Button
            type="text"
            style={{
              ...quickActionButtonStyle,
              color: countdownText ? token.colorPrimary : token.colorTextSecondary,
            }}
          >
            <CloudUpload size={14} />
            <span style={actionLabelStyle}>{t('titlebar.quickBackup')}</span>
            <span style={actionMetaStyle}>{countdownText ?? t('titlebar.noBackupYet')}</span>
          </Button>
        </Popover>

        <Button type="text" style={quickActionButtonStyle} onClick={handleReload}>
          <RotateCcw size={14} />
          <span style={actionLabelStyle}>{t('desktop.reloadPage')}</span>
          <span style={actionMetaStyle}>Ctrl+R</span>
        </Button>

        <Dropdown menu={{ items: githubMenuItems, onClick: handleGithubClick }} trigger={['click']} placement="topLeft" destroyOnHidden>
          <Button type="text" style={quickActionButtonStyle}>
            <Github size={14} />
            <span style={actionLabelStyle}>GitHub</span>
            <span style={actionMetaStyle}>{t('titlebar.submitFeature')}</span>
          </Button>
        </Dropdown>
      </div>
    </div>
  );

  return (
    <>
      <Popover
        trigger="click"
        placement="rightBottom"
        open={menuOpen}
        onOpenChange={setMenuOpen}
        content={popoverContent}
        arrow={false}
        destroyTooltipOnHide
      >
        <Tooltip title={profile.name || t('userProfile.title')} placement="right">
          <button style={triggerButtonStyle}>
            {renderUserAvatar()}
          </button>
        </Tooltip>
      </Popover>

      <UserProfileModal open={profileModalOpen} onClose={() => setProfileModalOpen(false)} />
    </>
  );
}
