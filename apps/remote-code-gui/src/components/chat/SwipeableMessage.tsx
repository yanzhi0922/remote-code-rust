import React, { useState, useRef, useCallback } from 'react';

interface SwipeableMessageProps {
  children: React.ReactNode;
  onReply?: () => void;
  onCopy?: () => void;
  onDelete?: () => void;
  actions?: Array<{
    label: string;
    icon: React.ReactNode;
    className?: string;
    onClick: () => void;
  }>;
}

export function SwipeableMessage({ children, onReply, onCopy, onDelete, actions }: SwipeableMessageProps) {
  const [offset, setOffset] = useState(0);
  const startX = useRef(0);
  const isDragging = useRef(false);
  // Track current offset via ref to avoid stale closure in handleTouchEnd.
  const offsetRef = useRef(0);
  offsetRef.current = offset;

  const handleTouchStart = useCallback((e: React.TouchEvent) => {
    startX.current = e.touches[0].clientX;
    isDragging.current = true;
  }, []);

  const handleTouchMove = useCallback((e: React.TouchEvent) => {
    if (!isDragging.current) return;
    const deltaX = e.touches[0].clientX - startX.current;
    if (deltaX > 0) return;
    setOffset(Math.max(-160, deltaX));
  }, []);

  const handleTouchEnd = useCallback(() => {
    isDragging.current = false;
    if (offsetRef.current < -60) {
      setOffset(-160);
    } else {
      setOffset(0);
    }
  }, []);

  const handleAction = useCallback((action: () => void) => {
    setOffset(0);
    action();
  }, []);

  const defaultActions = [
    ...(onReply ? [{ key: 'reply', label: '回复', className: 'bg-rc-accent-primary', icon: (
      <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6" />
      </svg>
    ), onClick: onReply }] : []),
    ...(onCopy ? [{ key: 'copy', label: '复制', className: 'bg-rc-accent-info', icon: (
      <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
      </svg>
    ), onClick: onCopy }] : []),
    ...(onDelete ? [{ key: 'delete', label: '删除', className: 'bg-rc-accent-danger', icon: (
      <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
      </svg>
    ), onClick: onDelete }] : []),
  ];

  const visibleActions = actions?.length ? actions.map((a, i) => ({ ...a, key: `custom-${i}` })) : defaultActions;
  const actionWidth = visibleActions.length * 56;

  return (
    <div className="relative overflow-hidden">
      {/* Action buttons behind the message */}
      <div
        className="absolute right-0 top-0 h-full flex items-stretch"
        style={{ width: actionWidth, transform: `translateX(${offset + actionWidth}px)` }}
      >
        {visibleActions.map((action) => (
          <button
            key={action.key}
            onClick={() => handleAction(action.onClick)}
            className={`flex flex-col items-center justify-center w-14 gap-0.5 text-rc-text-inverse text-xs font-medium active:opacity-80 ${action.className || ''}`}
            aria-label={action.label}
          >
            {action.icon}
            <span>{action.label}</span>
          </button>
        ))}
      </div>

      {/* Message content */}
      <div
        className="transition-transform duration-200"
        style={{ transform: `translateX(${offset}px)` }}
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
      >
        {children}
      </div>
    </div>
  );
}

interface PullToRefreshContainerProps {
  onRefresh: () => Promise<void>;
  children: React.ReactNode;
  className?: string;
}

export function PullToRefreshContainer({ onRefresh, children, className }: PullToRefreshContainerProps) {
  const [pulling, setPulling] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [startY, setStartY] = useState(0);
  const [pullDist, setPullDist] = useState(0);
  const scrollTop = useRef(0);

  const handleTouchStart = (e: React.TouchEvent) => {
    scrollTop.current = (e.currentTarget as HTMLElement).scrollTop;
    setStartY(e.touches[0].clientY);
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    if (refreshing) return;
    if (scrollTop.current !== 0) return;
    const diff = e.touches[0].clientY - startY;
    if (diff > 0) {
      setPulling(true);
      setPullDist(Math.min(diff, 100));
    }
  };

  const handleTouchEnd = async () => {
    if (!pulling) return;
    setPulling(false);
    if (pullDist > 60 && !refreshing) {
      setRefreshing(true);
      try {
        await onRefresh();
      } catch {
        // Silently handle refresh failures to prevent UI freeze.
      }
      setRefreshing(false);
    }
    setPullDist(0);
  };

  return (
    <div
      className={`overflow-auto ${className || ''}`}
      onTouchStart={handleTouchStart}
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
    >
      {pulling && !refreshing && pullDist > 0 && (
        <div
          className="flex items-center justify-center transition-all duration-200 overflow-hidden"
          style={{ height: Math.max(0, pullDist - 20) }}
        >
          <span className="text-sm text-rc-text-secondary">{pullDist > 60 ? '松开刷新' : '下拉刷新'}</span>
        </div>
      )}

      {refreshing && (
        <div role="status" className="flex items-center justify-center py-4">
          <div className="w-5 h-5 rounded-full border-2 border-rc-accent-primary border-t-transparent animate-spin" />
        </div>
      )}

      {children}
    </div>
  );
}