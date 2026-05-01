import React from 'react';
import { Sidebar } from './Sidebar';
import { Header } from './Header';

interface LayoutProps {
  children: React.ReactNode;
}

export function Layout({ children }: LayoutProps) {
  return (
    <div className="flex h-screen w-screen overflow-hidden bg-rc-bg-primary text-rc-text-primary font-sans">
      <Sidebar />
      <div className="flex-1 flex flex-col h-full bg-rc-bg-primary relative">
        <Header />
        <main className="flex-1 w-full relative flex flex-col min-h-0">
          {children}
        </main>
      </div>
    </div>
  );
}
