import { lazy, Suspense, useState } from 'react';
import { ActivityBar, type ActivityTab } from './ActivityBar';
import { Sidebar } from './Sidebar';
import { StatusBar } from './StatusBar';

const LazySettingsPanel = lazy(() =>
  import('./SettingsPanel').then((module) => ({ default: module.SettingsPanel })),
);

interface LayoutProps {
  children: React.ReactNode;
}

export function Layout({ children }: LayoutProps) {
  const [activeTab, setActiveTab] = useState<ActivityTab>('chat');
  const [settingsOpen, setSettingsOpen] = useState(false);

  const handleTabChange = (tab: ActivityTab) => {
    if (tab === 'settings') {
      setSettingsOpen(true);
      return;
    }
    setActiveTab(tab);
  };

  const showSidebar = activeTab === 'chat';

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-rc-bg-base text-rc-text-primary font-sans antialiased">
      <div className="flex min-h-0 flex-1">
        <ActivityBar activeTab={activeTab} onTabChange={handleTabChange} />

        {showSidebar && (
          <Sidebar />
        )}

        <main className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
          {children}
        </main>
      </div>

      <StatusBar />

      <Suspense fallback={null}>
        {settingsOpen && (
          <LazySettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />
        )}
      </Suspense>
    </div>
  );
}
