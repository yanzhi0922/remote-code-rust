/**
 * Badge — 徽章组件。
 *
 * 胶囊样式，支持五种颜色变体和可选左侧圆点指示器。
 */

import { type ReactNode } from 'react';
import { cn } from '@/lib/utils';

export interface BadgeProps {
  variant?: 'default' | 'success' | 'warning' | 'error' | 'info';
  size?: 'sm' | 'md';
  children: ReactNode;
  dot?: boolean;
  className?: string;
}

const variantStyles: Record<NonNullable<BadgeProps['variant']>, string> = {
  default: 'bg-slate-100 text-slate-700',
  success: 'bg-emerald-50 text-emerald-700',
  warning: 'bg-amber-50 text-amber-700',
  error: 'bg-red-50 text-red-700',
  info: 'bg-blue-50 text-blue-700',
};

const dotColors: Record<NonNullable<BadgeProps['variant']>, string> = {
  default: 'bg-slate-500',
  success: 'bg-emerald-500',
  warning: 'bg-amber-500',
  error: 'bg-red-500',
  info: 'bg-blue-500',
};

const sizeStyles: Record<NonNullable<BadgeProps['size']>, string> = {
  sm: 'px-2 py-0.5 text-xs',
  md: 'px-2.5 py-1 text-sm',
};

export function Badge({
  variant = 'default',
  size = 'md',
  children,
  dot = false,
  className,
}: BadgeProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full font-medium',
        variantStyles[variant],
        sizeStyles[size],
        className,
      )}
      data-testid="badge"
    >
      {dot && (
        <span
          className={cn('h-1.5 w-1.5 rounded-full', dotColors[variant])}
          data-testid="badge-dot"
        />
      )}
      {children}
    </span>
  );
}
