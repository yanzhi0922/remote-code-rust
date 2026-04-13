import React from 'react';
import { Sidebar } from './Sidebar';
import { Header } from './Header';

interface LayoutProps {
  children: React.ReactNode;
}

export function Layout({ children }: LayoutProps) {
  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#f8f7f4] text-slate-800 font-sans">
      <Sidebar />
      <div className="flex-1 flex flex-col h-full bg-[#f8f7f4] relative">
        <Header />
        <main className="flex-1 w-full relative flex flex-col min-h-0">
          {children}
        </main>
      </div>
    </div>
  );
}
