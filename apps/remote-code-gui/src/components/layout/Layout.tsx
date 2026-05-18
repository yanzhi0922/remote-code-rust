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
    <div className="flex h-screen w-screen overflow-hidden bg-[radial-gradient(circle_at_20%_0%,rgba(37,99,235,0.12),transparent_34%),radial-gradient(circle_at_92%_18%,rgba(8,145,178,0.12),transparent_30%),linear-gradient(135deg,#f2f6fb_0%,#f8fafc_50%,#eef7f4_100%)] text-rc-text-primary font-sans antialiased dark:bg-[radial-gradient(circle_at_20%_0%,rgba(37,99,235,0.18),transparent_34%),radial-gradient(circle_at_92%_18%,rgba(20,184,166,0.12),transparent_30%),linear-gradient(135deg,#07111f_0%,#0b1220_50%,#081817_100%)]">
      <ActivityBar activeTab={activeTab} onTabChange={handleTabChange} />

      {showSidebar && (
        <div className="animate-[fadeIn_200ms_ease]">
          <Sidebar />
        </div>
      )}

      <div className="flex h-full min-w-0 flex-1 flex-col py-4 pr-4">
        <main className="relative flex min-h-0 w-full flex-1 flex-col overflow-hidden rounded-lg border border-white/80 bg-white/70 shadow-[0_22px_60px_rgba(15,23,42,0.12)] backdrop-blur-xl dark:border-rc-border-primary dark:bg-rc-bg-base/70">
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
