import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ActivityBar, type ActivityTab } from './ActivityBar';
import { BottomWorkbench } from './BottomWorkbench';
import { Sidebar } from './Sidebar';
import { SessionInspector } from './SessionInspector';
import { StatusBar } from './StatusBar';
import { CommandPalette } from '../shared/CommandPalette';
import { SessionSwitcher } from './SessionSwitcher';
import type { SettingsTab } from './SettingsPanel';
import { useKeyboardShortcuts } from '../../lib/useKeyboardShortcuts';
import { useWorkbenchLayout } from '../../lib/useWorkbenchLayout';
import { useAppStore } from '../../stores/useAppStore';
import { useTheme } from '../design/ThemeProvider';
import { PanelBottomOpen, PanelLeftClose, PanelLeftOpen, PanelRightClose, PanelRightOpen } from 'lucide-react';

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
  const workbenchLayout = useWorkbenchLayout();
  const {
    state: layoutState,
    update: updateLayout,
    toggleSidebar,
    toggleInspector,
    toggleBottom,
    openBottomTab,
    setBottomHeight,
  } = workbenchLayout;

  const { toggle: toggleTheme } = useTheme();
  const createSession = useAppStore((state) => state.createSession);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const pickFolderAndAddProject = useAppStore((state) => state.pickFolderAndAddProject);
  const sessions = useAppStore((state) => state.sessions);
  const activeSessionId = useAppStore((state) => state.activeSessionId);

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

  const handleToggleSidebar = useCallback(() => {
    toggleSidebar();
    setActiveTab('chat');
  }, [toggleSidebar]);

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

  // ── Centralized keyboard shortcuts ───────────────────────────────────
  const shortcuts = useMemo(
    () => [
      {
        key: 'k',
        modifier: 'mod' as const,
        action: () => setCommandPaletteOpen((prev) => !prev),
        description: 'Toggle command palette',
      },
      {
        key: 'n',
        modifier: 'mod' as const,
        action: () => void createSession(undefined, activeProjectPath ?? undefined),
        description: 'New session',
        enabled: !commandPaletteOpen,
      },
      {
        key: ',',
        modifier: 'mod' as const,
        action: handleOpenSettings,
        description: 'Open settings',
        enabled: !commandPaletteOpen,
      },
      // ── Windows / cross-platform extras ─────────────────────────────
      {
        key: 'b',
        modifier: 'mod' as const,
        action: handleToggleSidebar,
        description: 'Toggle sidebar',
        enabled: !commandPaletteOpen,
      },
      {
        key: 'e',
        modifier: 'ctrl+shift' as const,
        action: () => {
          setSettingsInitialTab('provider');
          setSettingsOpen(true);
          setActiveTab('settings');
        },
        description: 'Open explorer',
        enabled: !commandPaletteOpen,
      },
      {
        key: 'm',
        modifier: 'ctrl+shift' as const,
        action: handleOpenMcp,
        description: 'Open MCP management',
        enabled: !commandPaletteOpen,
      },
      {
        key: 'j',
        modifier: 'mod' as const,
        action: () => toggleBottom('terminal'),
        description: 'Toggle bottom workbench',
        enabled: !commandPaletteOpen,
      },
      {
        key: 'i',
        modifier: 'mod' as const,
        action: toggleInspector,
        description: 'Toggle inspector',
        enabled: !commandPaletteOpen,
      },
      {
        key: 'l',
        modifier: 'mod' as const,
        action: () => {
          setActiveTab('chat');
        },
        description: 'Focus chat',
        enabled: !commandPaletteOpen,
      },
    ],
    [
      commandPaletteOpen,
      createSession,
      activeProjectPath,
      handleOpenSettings,
      handleOpenMcp,
      handleToggleSidebar,
      toggleBottom,
      toggleInspector,
    ],
  );

  useKeyboardShortcuts(shortcuts);

  const showSidebar = activeTab === 'chat' && !layoutState.sidebarCollapsed;
  const showCollapsedSidebar = activeTab === 'chat' && layoutState.sidebarCollapsed;
  const showInspector = !layoutState.inspectorCollapsed;

  return (
    <div className="codex-desktop-shell relative flex h-dvh w-screen flex-col overflow-hidden p-4 text-rc-text-primary font-sans antialiased">
      <div className="codex-window-surface relative flex min-h-0 flex-1 overflow-hidden">
        <ActivityBar activeTab={activeTab} onTabChange={(tab) => {
          if (tab === 'settings' || tab === 'mcp') {
            setSettingsInitialTab(tab === 'mcp' ? 'mcp' : 'provider');
            setSettingsOpen(true);
            setActiveTab(tab);
            return;
          }
          setActiveTab(tab);
        }} />

        {showSidebar && (
          <Sidebar />
        )}

        {showCollapsedSidebar && (
          <aside
            aria-label={t('layout.collapsedSidebar')}
            className="flex w-12 shrink-0 flex-col items-center border-r border-rc-border-primary bg-rc-bg-surface pt-20"
          >
            <button
              type="button"
              onClick={toggleSidebar}
              className="flex h-8 w-8 items-center justify-center rounded-full text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
              aria-label={t('layout.expandSidebar')}
              title={t('layout.expandSidebar')}
            >
              <PanelLeftOpen size={16} />
            </button>
          </aside>
        )}

        <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
          <main
            aria-label="Agent conversation workbench"
            className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-rc-bg-chat"
          >
            <div className="absolute right-5 top-5 z-20 flex items-center gap-1 rounded-lg border border-rc-border-primary bg-rc-bg-elevated p-1 shadow-sm">
              <button
                type="button"
                onClick={toggleSidebar}
                className="codex-floating-control hidden border-transparent bg-transparent shadow-none lg:inline-flex"
                aria-label={layoutState.sidebarCollapsed ? t('layout.expandSidebar') : t('layout.collapseSidebar')}
              >
                {layoutState.sidebarCollapsed ? <PanelLeftOpen size={13} /> : <PanelLeftClose size={13} />}
              </button>
              <button
                type="button"
                onClick={toggleInspector}
                className="codex-floating-control hidden border-transparent bg-transparent shadow-none lg:inline-flex"
                aria-label={layoutState.inspectorCollapsed ? t('layout.expandInspector') : t('layout.collapseInspector')}
              >
                {layoutState.inspectorCollapsed ? <PanelRightOpen size={13} /> : <PanelRightClose size={13} />}
              </button>
              <button
                type="button"
                onClick={() => toggleBottom()}
                className="codex-floating-control inline-flex border-transparent bg-transparent shadow-none"
                aria-label={layoutState.bottomOpen ? t('layout.collapseBottom') : t('layout.expandBottom')}
              >
                <PanelBottomOpen size={13} />
              </button>
            </div>
            {children}
          </main>
          <BottomWorkbench
            open={layoutState.bottomOpen}
            activeTab={layoutState.bottomTab}
            height={layoutState.bottomHeight}
            onTabChange={openBottomTab}
            onClose={() => updateLayout({ bottomOpen: false })}
            onHeightChange={setBottomHeight}
          />

          {showInspector && (
            <div className="absolute bottom-5 right-5 top-16 z-30 hidden xl:block">
              <SessionInspector />
            </div>
          )}
        </div>
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
