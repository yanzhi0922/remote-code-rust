import React, { useState, Suspense, lazy, useCallback } from 'react';
import { MobileHeader, MobileBottomNav, MobileDrawer, MobileSheet } from './MobileLayout';
import { Sidebar } from './Sidebar';

const LazySettingsPanel = lazy(() =>
  import('./SettingsPanel').then((m) => ({ default: m.SettingsPanel })),
);

interface MobileAppLayoutProps {
  children: React.ReactNode;
  title?: string;
}

export function MobileAppLayout({ children, title = 'Remote Code' }: MobileAppLayoutProps) {
  const [activeTab, setActiveTab] = useState('chat');
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const handleTabChange = useCallback((tab: string) => {
    if (tab === 'settings') {
      setSettingsOpen(true);
      return;
    }
    if (tab === 'menu') {
      setSidebarOpen(true);
      return;
    }
    setActiveTab(tab);
  }, []);

  const tabs = [
    {
      id: 'menu',
      label: '菜单',
      icon: (
        <svg fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M4 6h16M4 12h16M4 18h16" />
        </svg>
      ),
    },
    {
      id: 'chat',
      label: '对话',
      icon: (
        <svg fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.86 9.86 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
        </svg>
      ),
    },
    {
      id: 'settings',
      label: '设置',
      icon: (
        <svg fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M12 15a3 3 0 100-6 3 3 0 000 6z" />
          <path strokeLinecap="round" strokeLinejoin="round" d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
        </svg>
      ),
    },
  ];

  return (
    <div className="flex flex-col h-screen w-screen bg-rc-bg-base text-rc-text-primary font-sans antialiased">
      <MobileHeader
        title={title}
        onMenuClick={() => setSidebarOpen(true)}
      />

      <main className="flex-1 flex flex-col min-h-0 overflow-hidden">
        {children}
      </main>

      <MobileBottomNav activeTab={activeTab} onTabChange={handleTabChange} tabs={tabs} />

      {/* Sidebar Drawer */}
      <MobileDrawer open={sidebarOpen} onClose={() => setSidebarOpen(false)} position="left">
        <div className="flex flex-col h-full">
          <div className="flex items-center justify-between p-4 border-b border-rc-border-primary">
            <div className="flex items-center gap-3">
              <div className="h-8 w-8 rounded-lg bg-gradient-to-br from-rc-accent-primary to-purple-500 flex items-center justify-center shadow-md">
                <span className="text-white text-sm font-bold">RC</span>
              </div>
              <span className="font-semibold text-rc-text-primary">Remote Code</span>
            </div>
          </div>
          <div className="flex-1 overflow-auto">
            <Sidebar />
          </div>
        </div>
      </MobileDrawer>

      {/* Settings Sheet */}
      <Suspense fallback={null}>
        {settingsOpen && (
          <MobileSheet
            open={settingsOpen}
            onClose={() => setSettingsOpen(false)}
            title="设置"
          >
            <LazySettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />
          </MobileSheet>
        )}
      </Suspense>
    </div>
  );
}