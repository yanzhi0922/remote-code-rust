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
    <div className="flex h-screen w-screen overflow-hidden bg-rc-bg-base text-rc-text-primary font-sans antialiased">
      {/* Activity Bar — refined with subtle border */}
      <ActivityBar activeTab={activeTab} onTabChange={handleTabChange} />

      {/* Sidebar — with smooth transition */}
      {showSidebar && (
        <div className="animate-[fadeIn_200ms_ease]">
          <Sidebar />
        </div>
      )}

      {/* Main Content Area */}
      <div className="flex flex-1 flex-col h-full min-w-0 border-l border-rc-border-primary">
        <main className="flex-1 w-full relative flex flex-col min-h-0">
          {children}
        </main>
        <StatusBar />
      </div>

      {/* Settings Modal */}
      <Suspense fallback={null}>
        {settingsOpen && (
          <LazySettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />
        )}
      </Suspense>
    </div>
  );
}