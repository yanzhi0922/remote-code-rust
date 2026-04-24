import type { ReactNode } from 'react';

export interface MessageRowProps {
  children: ReactNode;
  role: 'user' | 'assistant' | 'system';
  className?: string;
}

export function MessageRow({ children, role, className }: MessageRowProps) {
  const bgClass = role === 'user'
    ? 'bg-slate-50'
    : role === 'system'
      ? 'bg-amber-50'
      : '';

  return (
    <div
      data-testid={`message-row-${role}`}
      className={`px-4 py-3 ${bgClass} ${className ?? ''}`}
    >
      {children}
    </div>
  );
}
