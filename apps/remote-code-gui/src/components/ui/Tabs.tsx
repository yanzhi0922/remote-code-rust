/**
 * Tabs — 标签页组件。
 *
 * 水平标签栏，支持活动下划线、图标和键盘导航。
 */

import { type ReactNode, useCallback, useRef } from 'react';
import { cn } from '@/lib/utils';

export interface Tab {
  key: string;
  label: string;
  icon?: ReactNode;
}

export interface TabsProps {
  tabs: Tab[];
  activeKey: string;
  onChange: (key: string) => void;
  className?: string;
}

export function Tabs({ tabs, activeKey, onChange, className }: TabsProps) {
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent, index: number) => {
      let nextIndex: number | null = null;

      if (e.key === 'ArrowRight') {
        nextIndex = (index + 1) % tabs.length;
      } else if (e.key === 'ArrowLeft') {
        nextIndex = (index - 1 + tabs.length) % tabs.length;
      }

      if (nextIndex !== null) {
        e.preventDefault();
        tabRefs.current[nextIndex]?.focus();
        onChange(tabs[nextIndex].key);
      }
    },
    [tabs, onChange],
  );

  return (
    <div
      className={cn('flex border-b border-slate-200', className)}
      role="tablist"
      data-testid="tabs"
    >
      {tabs.map((tab, index) => {
        const isActive = tab.key === activeKey;
        return (
          <button
            key={tab.key}
            ref={(el) => {
              tabRefs.current[index] = el;
            }}
            role="tab"
            aria-selected={isActive}
            tabIndex={isActive ? 0 : -1}
            onClick={() => onChange(tab.key)}
            onKeyDown={(e) => handleKeyDown(e, index)}
            className={cn(
              'inline-flex items-center gap-2 border-b-2 px-4 py-2.5 text-sm font-medium transition-colors',
              isActive
                ? 'border-slate-800 text-slate-900'
                : 'border-transparent text-slate-500 hover:border-slate-300 hover:text-slate-700',
            )}
            data-testid={`tab-${tab.key}`}
          >
            {tab.icon && (
              <span className="flex-shrink-0" data-testid={`tab-icon-${tab.key}`}>
                {tab.icon}
              </span>
            )}
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}
