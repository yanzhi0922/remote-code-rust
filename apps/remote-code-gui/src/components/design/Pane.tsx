import { useState } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface PaneProps {
  title?: string;
  collapsible?: boolean;
  defaultCollapsed?: boolean;
  className?: string;
  children: React.ReactNode;
}

export function Pane({ title, collapsible = false, defaultCollapsed = false, className, children }: PaneProps) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed);

  return (
    <div
      data-testid="pane"
      className={cn('rounded-lg border border-slate-200 bg-white', className)}
    >
      {title && (
        <div
          className={cn(
            'flex items-center justify-between border-b border-slate-200 px-4 py-3',
            collapsible ? 'cursor-pointer select-none' : ''
          )}
          onClick={collapsible ? () => setCollapsed(!collapsed) : undefined}
          data-testid="pane-header"
        >
          <h3 data-testid="pane-title" className="text-sm font-medium text-slate-700">
            {title}
          </h3>
          {collapsible && (
            <span data-testid="pane-collapse-icon">
              {collapsed ? (
                <ChevronRight className="h-4 w-4 text-slate-400" />
              ) : (
                <ChevronDown className="h-4 w-4 text-slate-400" />
              )}
            </span>
          )}
        </div>
      )}
      {(!collapsible || !collapsed) && (
        <div data-testid="pane-content" className="px-4 py-3">
          {children}
        </div>
      )}
    </div>
  );
}
