/**
 * Divider — 分割线组件。
 *
 * 支持水平/垂直方向，可选中间标签。
 */

import { cn } from '@/lib/utils';

export interface DividerProps {
  orientation?: 'horizontal' | 'vertical';
  label?: string;
  className?: string;
}

export function Divider({
  orientation = 'horizontal',
  label,
  className,
}: DividerProps) {
  if (orientation === 'vertical') {
    return (
      <div
        className={cn('inline-flex h-full items-center', className)}
        role="separator"
        aria-orientation="vertical"
        data-testid="divider"
      >
        <div className="h-full w-px bg-slate-200" />
      </div>
    );
  }

  if (label) {
    return (
      <div
        className={cn('flex items-center gap-3', className)}
        role="separator"
        data-testid="divider"
      >
        <div className="flex-1 border-t border-slate-200" />
        <span
          className="text-xs font-medium text-slate-400"
          data-testid="divider-label"
        >
          {label}
        </span>
        <div className="flex-1 border-t border-slate-200" />
      </div>
    );
  }

  return (
    <div
      className={cn('border-t border-slate-200', className)}
      role="separator"
      aria-orientation="horizontal"
      data-testid="divider"
    />
  );
}
