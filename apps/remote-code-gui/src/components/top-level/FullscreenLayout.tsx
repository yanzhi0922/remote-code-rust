import type { ReactNode } from 'react';

export interface FullscreenLayoutProps {
  children: ReactNode;
  header?: ReactNode;
  footer?: ReactNode;
}

export function FullscreenLayout({ children, header, footer }: FullscreenLayoutProps) {
  return (
    <div data-testid="fullscreen-layout" className="flex h-screen flex-col bg-white">
      {header && (
        <header data-testid="fullscreen-header" className="shrink-0 border-b border-slate-200">
          {header}
        </header>
      )}
      <main data-testid="fullscreen-main" className="flex-1 overflow-auto">
        {children}
      </main>
      {footer && (
        <footer data-testid="fullscreen-footer" className="shrink-0 border-t border-slate-200">
          {footer}
        </footer>
      )}
    </div>
  );
}
