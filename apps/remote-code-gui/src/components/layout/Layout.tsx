import { lazy, Suspense, useState } from 'react';
import { ActivityBar, type ActivityTab } from './ActivityBar';
import { Sidebar } from './Sidebar';
import { StatusBar } from './StatusBar';
import { SessionInspector } from './SessionInspector';
import type { SettingsTab } from './SettingsPanel';

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

  const handleTabChange = (tab: ActivityTab) => {
    if (tab === 'settings' || tab === 'mcp') {
      setSettingsInitialTab(tab === 'mcp' ? 'mcp' : 'provider');
      setSettingsOpen(true);
      setActiveTab(tab);
      return;
    }
    setActiveTab(tab);
  };

  const showSidebar = activeTab === 'chat';

  return (
    <div className="flex min-h-dvh w-screen flex-col overflow-hidden bg-rc-bg-base text-rc-text-primary font-sans antialiased">
      <div className="flex min-h-0 flex-1">
        <ActivityBar activeTab={activeTab} onTabChange={handleTabChange} />

        {showSidebar && (
          <Sidebar />
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
    </div>
  );
}
