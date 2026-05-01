import React, { lazy, Suspense, useState } from 'react';
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
    <div className="flex h-screen w-screen overflow-hidden bg-rc-bg-primary text-rc-text-primary font-sans">
      {/* Activity Bar */}
      <ActivityBar activeTab={activeTab} onTabChange={handleTabChange} />

      {/* Sidebar (only for chat tab) */}
      {showSidebar && <Sidebar />}

      {/* Main Content Area */}
      <div className="flex flex-1 flex-col h-full min-w-0">
        <main className="flex-1 w-full relative flex flex-col min-h-0">
          {children}
        </main>
        <StatusBar />
      </div>

      {/* Settings Modal */}
      <Suspense fallback={null}>
        {settingsOpen && <LazySettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />}
      </Suspense>
    </div>
  );
}
