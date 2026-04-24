import { useRef, useEffect, type ReactNode } from 'react';

export interface VirtualMessageListProps {
  children: ReactNode[];
  overscan?: number;
  className?: string;
}

export function VirtualMessageList({ children, className }: VirtualMessageListProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [children.length]);

  return (
    <div
      ref={containerRef}
      data-testid="virtual-message-list"
      className={`flex-1 overflow-y-auto ${className ?? ''}`}
    >
      {children.map((child, i) => (
        <div key={i} data-testid={`virtual-message-${i}`}>
          {child}
        </div>
      ))}
    </div>
  );
}
