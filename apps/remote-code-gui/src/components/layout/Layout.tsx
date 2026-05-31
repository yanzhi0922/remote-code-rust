import { lazy, Suspense, useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ActivityBar, type ActivityTab } from './ActivityBar';
import { Sidebar } from './Sidebar';
import { StatusBar } from './StatusBar';
import { SessionInspector } from './SessionInspector';
import { CommandPalette } from '../shared/CommandPalette';
import { SessionSwitcher } from './SessionSwitcher';
import type { SettingsTab } from './SettingsPanel';
import { useResizableWidth } from '../../lib/useResizableWidth';
import { useAppStore } from '../../stores/useAppStore';
import { useTheme } from '../design/ThemeProvider';

const LazySettingsPanel = lazy(() =>
  import('./SettingsPanel').then((module) => ({ default: module.SettingsPanel })),
);

interface LayoutProps {
  children: React.ReactNode;
  initialSettingsOpen?: boolean;
  initialSettingsTab?: SettingsTab;
}

export function Layout({
  children,
  initialSettingsOpen = false,
  initialSettingsTab = 'provider',
}: LayoutProps) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<ActivityTab>(initialSettingsOpen && initialSettingsTab === 'mcp' ? 'mcp' : 'chat');
  const [settingsOpen, setSettingsOpen] = useState(initialSettingsOpen);
  const [settingsInitialTab, setSettingsInitialTab] = useState<SettingsTab>(initialSettingsTab);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const { handleMouseDown } = useResizableWidth();

  const { toggle: toggleTheme } = useTheme();
  const createSession = useAppStore((state) => state.createSession);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const pickFolderAndAddProject = useAppStore((state) => state.pickFolderAndAddProject);
  const sessions = useAppStore((state) => state.sessions);
  const activeSessionId = useAppStore((state) => state.activeSessionId);

  const handleTabChange = (tab: ActivityTab) => {
    if (tab === 'settings' || tab === 'mcp') {
      setSettingsInitialTab(tab === 'mcp' ? 'mcp' : 'provider');
      setSettingsOpen(true);
      setActiveTab(tab);
      return;
    }
    setActiveTab(tab);
  };

  const handleOpenSettings = useCallback(() => {
    setSettingsInitialTab('provider');
    setSettingsOpen(true);
    setActiveTab('settings');
  }, []);

  const handleOpenMcp = useCallback(() => {
    setSettingsInitialTab('mcp');
    setSettingsOpen(true);
    setActiveTab('mcp');
  }, []);

  const handleNewSession = useCallback(() => {
    void createSession(undefined, activeProjectPath ?? undefined);
  }, [createSession, activeProjectPath]);

  const handleAddProject = useCallback(() => {
    void pickFolderAndAddProject();
  }, [pickFolderAndAddProject]);

  // Listen for custom navigation events (e.g., from context menu "前往配置")
  useEffect(() => {
    const handleNavigateToSettings = () => {
      setSettingsInitialTab('provider');
      setSettingsOpen(true);
      setActiveTab('settings');
    };
    window.addEventListener('navigate-to-settings', handleNavigateToSettings);
    return () => window.removeEventListener('navigate-to-settings', handleNavigateToSettings);
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const isMod = event.metaKey || event.ctrlKey;

      if (isMod && event.key === 'k') {
        event.preventDefault();
        setCommandPaletteOpen((prev) => !prev);
        return;
      }

      if (commandPaletteOpen) return;

      if (isMod && event.key === 'n') {
        event.preventDefault();
        void createSession(undefined, activeProjectPath ?? undefined);
        return;
      }

      if (isMod && event.key === ',') {
        event.preventDefault();
        handleOpenSettings();
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [commandPaletteOpen, createSession, activeProjectPath, handleOpenSettings]);

  const showSidebar = activeTab === 'chat';

  return (
    <div className="flex h-dvh w-screen flex-col overflow-hidden bg-rc-bg-base text-rc-text-primary font-sans antialiased">
      <div className="flex min-h-0 flex-1">
        <ActivityBar activeTab={activeTab} onTabChange={handleTabChange} />

        {showSidebar && (
          <>
            <Sidebar />
            <div
              className="resize-handle"
              onMouseDown={handleMouseDown}
              role="separator"
              aria-orientation="vertical"
              aria-label={t('layout.resizeSidebar')}
            />
          </>
        )}

        <main
          aria-label="Agent conversation workbench"
          className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-tl-xl border-l border-t border-rc-border-secondary bg-rc-bg-chat shadow-xs"
        >
          {children}
        </main>

        <SessionInspector />
      </div>

      <StatusBar />

      <Suspense fallback={null}>
        {settingsOpen && (
          <LazySettingsPanel
            open={settingsOpen}
            initialTab={settingsInitialTab}
            onClose={() => {
              setSettingsOpen(false);
              setActiveTab('chat');
            }}
          />
        )}
      </Suspense>

      <CommandPalette
        open={commandPaletteOpen}
        onClose={() => setCommandPaletteOpen(false)}
        onNewSession={handleNewSession}
        onAddProject={handleAddProject}
        onOpenSettings={handleOpenSettings}
        onOpenMcp={handleOpenMcp}
        onToggleTheme={toggleTheme}
      />

      <SessionSwitcher sessions={sessions} activeSessionId={activeSessionId} />
    </div>
  );
}
