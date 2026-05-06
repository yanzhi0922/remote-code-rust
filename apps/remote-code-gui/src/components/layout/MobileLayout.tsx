import React, { useState, useCallback } from 'react';

interface MobileHeaderProps {
  title: string;
  onMenuClick?: () => void;
  onBackClick?: () => void;
  showBack?: boolean;
  actions?: React.ReactNode;
}

export function MobileHeader({
  title,
  onMenuClick,
  onBackClick,
  showBack = false,
  actions,
}: MobileHeaderProps) {
  return (
    <header className="flex items-center justify-between h-14 px-4 bg-rc-bg-surface border-b border-rc-border-primary shrink-0">
      <div className="flex items-center gap-2">
        {showBack && (
          <button
            onClick={onBackClick}
            className="p-2 -ml-2 rounded-lg hover:bg-rc-bg-base active:bg-rc-bg-tertiary transition-colors"
            aria-label="返回"
          >
            <svg className="w-5 h-5 text-rc-text-primary" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
            </svg>
          </button>
        )}
        {!showBack && onMenuClick && (
          <button
            onClick={onMenuClick}
            className="p-2 -ml-2 rounded-lg hover:bg-rc-bg-base active:bg-rc-bg-tertiary transition-colors"
            aria-label="菜单"
          >
            <svg className="w-5 h-5 text-rc-text-primary" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M4 6h16M4 12h16M4 18h16" />
            </svg>
          </button>
        )}
        <h1 className="text-base font-semibold text-rc-text-primary truncate">{title}</h1>
      </div>
      {actions && <div className="flex items-center gap-1">{actions}</div>}
    </header>
  );
}

interface MobileBottomNavProps {
  activeTab: string;
  onTabChange: (tab: string) => void;
  tabs: Array<{ id: string; label: string; icon: React.ReactNode }>;
}

export function MobileBottomNav({ activeTab, onTabChange, tabs }: MobileBottomNavProps) {
  return (
    <nav className="flex items-center justify-around h-16 bg-rc-bg-surface border-t border-rc-border-primary shrink-0 safe-area-pb">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          onClick={() => onTabChange(tab.id)}
          className={`flex flex-col items-center justify-center w-full h-full gap-1 transition-colors ${
            activeTab === tab.id
              ? 'text-rc-accent-primary'
              : 'text-rc-text-secondary hover:text-rc-text-primary'
          }`}
        >
          <span className="w-6 h-6">{tab.icon}</span>
          <span className="text-xs font-medium">{tab.label}</span>
        </button>
      ))}
    </nav>
  );
}

interface MobileDrawerProps {
  open: boolean;
  onClose: () => void;
  children: React.ReactNode;
  position?: 'left' | 'right';
}

export function MobileDrawer({ open, onClose, children, position = 'left' }: MobileDrawerProps) {
  const [touchStartX, setTouchStartX] = useState(0);

  const handleTouchStart = (e: React.TouchEvent) => {
    setTouchStartX(e.touches[0].clientX);
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    const deltaX = e.touches[0].clientX - touchStartX;
    if (position === 'left' && deltaX < -50) {
      onClose();
    } else if (position === 'right' && deltaX > 50) {
      onClose();
    }
  };

  return (
    <>
      {/* Backdrop */}
      <div
        className={`fixed inset-0 bg-black/50 z-40 transition-opacity duration-300 ${
          open ? 'opacity-100' : 'opacity-0 pointer-events-none'
        }`}
        onClick={onClose}
      />

      {/* Drawer */}
      <div
        className={`fixed top-0 ${position === 'left' ? 'left-0' : 'right-0'} z-50 w-80 max-w-[85vw] h-full bg-rc-bg-surface shadow-2xl transform transition-transform duration-300 ${
          open ? 'translate-x-0' : position === 'left' ? '-translate-x-full' : 'translate-x-full'
        }`}
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
      >
        {children}
      </div>
    </>
  );
}

interface MobilePullToRefreshProps {
  onRefresh: () => Promise<void>;
  children: React.ReactNode;
}

export function MobilePullToRefresh({ onRefresh, children }: MobilePullToRefreshProps) {
  const [pulling, setPulling] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [startY, setStartY] = useState(0);
  const [pullDistance, setPullDistance] = useState(0);

  const handleTouchStart = (e: React.TouchEvent) => {
    if (refreshing) return;
    setStartY(e.touches[0].clientY);
    setPulling(true);
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    if (!pulling || refreshing) return;
    const currentY = e.touches[0].clientY;
    const diff = currentY - startY;
    if (diff > 0) {
      setPullDistance(Math.min(diff, 120));
    }
  };

  const handleTouchEnd = async () => {
    if (!pulling) return;
    setPulling(false);

    if (pullDistance > 80 && !refreshing) {
      setRefreshing(true);
      await onRefresh();
      setRefreshing(false);
    }
    setPullDistance(0);
  };

  return (
    <div
      className="flex-1 overflow-auto"
      onTouchStart={handleTouchStart}
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
    >
      {/* Pull indicator */}
      {pulling && !refreshing && pullDistance > 0 && (
        <div
          className="flex items-center justify-center py-2 text-xs text-rc-text-secondary"
          style={{ height: Math.max(0, pullDistance - 40) }}
        >
          {pullDistance > 80 ? '松开刷新' : '下拉刷新'}
        </div>
      )}

      {/* Refreshing spinner */}
      {refreshing && (
        <div className="flex items-center justify-center py-4">
          <div className="w-5 h-5 rounded-full border-2 border-rc-accent-primary border-t-transparent animate-spin" />
        </div>
      )}

      {children}
    </div>
  );
}

interface MobileSheetProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
}

export function MobileSheet({ open, onClose, title, children, footer }: MobileSheetProps) {
  return (
    <>
      {/* Backdrop */}
      <div
        className={`fixed inset-0 bg-black/50 z-40 transition-opacity duration-300 ${
          open ? 'opacity-100' : 'opacity-0 pointer-events-none'
        }`}
        onClick={onClose}
      />

      {/* Sheet */}
      <div
        className={`fixed bottom-0 left-0 right-0 z-50 max-h-[80vh] bg-rc-bg-surface rounded-t-2xl shadow-2xl transform transition-transform duration-300 ${
          open ? 'translate-y-0' : 'translate-y-full'
        }`}
      >
        <div className="flex items-center justify-between px-4 py-3 border-b border-rc-border-primary">
          <h2 className="text-base font-semibold text-rc-text-primary">{title}</h2>
          <button
            onClick={onClose}
            className="p-1 rounded-lg hover:bg-rc-bg-base"
            aria-label="关闭"
          >
            <svg className="w-5 h-5 text-rc-text-secondary" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div className="overflow-auto" style={{ maxHeight: 'calc(80vh - 56px)' }}>
          {children}
        </div>

        {footer && (
          <div className="px-4 py-3 border-t border-rc-border-primary bg-rc-bg-base">
            {footer}
          </div>
        )}
      </div>
    </>
  );
}