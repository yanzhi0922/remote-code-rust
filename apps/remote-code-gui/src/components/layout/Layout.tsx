import { lazy, Suspense, useCallback, useEffect, useState } from 'react';
import { ActivityBar, type ActivityTab } from './ActivityBar';
import { Sidebar } from './Sidebar';
import { StatusBar } from './StatusBar';
import { SessionInspector } from './SessionInspector';
import { CommandPalette } from '../shared/CommandPalette';
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
  const [activeTab, setActiveTab] = useState<ActivityTab>(initialSettingsOpen && initialSettingsTab === 'mcp' ? 'mcp' : 'chat');
  const [settingsOpen, setSettingsOpen] = useState(initialSettingsOpen);
  const [settingsInitialTab, setSettingsInitialTab] = useState<SettingsTab>(initialSettingsTab);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const { handleMouseDown } = useResizableWidth();

  const { toggle: toggleTheme } = useTheme();
  const createSession = useAppStore((state) => state.createSession);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const pickFolderAndAddProject = useAppStore((state) => state.pickFolderAndAddProject);

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
              aria-label="调整侧边栏宽度"
            />
          </>
        )}

        <main
          aria-label="Agent conversation workbench"
          className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-rc-bg-chat"
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
    </div>
  );
}
