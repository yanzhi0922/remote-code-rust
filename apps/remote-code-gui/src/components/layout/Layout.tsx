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
    <div className="flex h-screen w-screen overflow-hidden bg-[linear-gradient(135deg,#eef3ff_0%,#f7fafc_46%,#eefaf6_100%)] text-rc-text-primary font-sans antialiased dark:bg-[linear-gradient(135deg,#07111f_0%,#0b1220_48%,#081817_100%)]">
      <ActivityBar activeTab={activeTab} onTabChange={handleTabChange} />

      {showSidebar && (
        <div className="animate-[fadeIn_200ms_ease]">
          <Sidebar />
        </div>
      )}

      <div className="flex h-full min-w-0 flex-1 flex-col py-5 pr-5">
        <main className="relative flex min-h-0 w-full flex-1 flex-col overflow-hidden rounded-xl border border-white/80 bg-white/60 shadow-[0_24px_70px_rgba(15,23,42,0.12)] backdrop-blur-xl dark:border-rc-border-primary dark:bg-rc-bg-base/70">
          {children}
        </main>
        <StatusBar />
      </div>

      <Suspense fallback={null}>
        {settingsOpen && (
          <LazySettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />
        )}
      </Suspense>
    </div>
  );
}
