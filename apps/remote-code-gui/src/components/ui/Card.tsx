/**
 * Card — 卡片容器组件。
 *
 * 白色背景 + 圆角 + 阴影，支持悬停效果和选中边框。
 */

import { type ReactNode } from 'react';
import { cn } from '@/lib/utils';

export interface CardProps {
  children: ReactNode;
  padding?: 'none' | 'sm' | 'md' | 'lg';
  hover?: boolean;
  selected?: boolean;
  onClick?: () => void;
  className?: string;
}

const paddingStyles: Record<NonNullable<CardProps['padding']>, string> = {
  none: '',
  sm: 'p-3',
  md: 'p-4',
  lg: 'p-6',
};

export function Card({
  children,
  padding = 'md',
  hover = false,
  selected = false,
  onClick,
  className,
}: CardProps) {
  const Component = onClick ? 'button' : 'div';

  return (
    <Component
      {...(onClick ? { onClick, type: 'button' as const } : {})}
      className={cn(
        'rounded-2xl bg-white shadow-sm',
        paddingStyles[padding],
        hover && 'transition-shadow hover:shadow-md',
        selected && 'ring-2 ring-slate-800',
        onClick && 'cursor-pointer text-left',
        className,
      )}
      data-testid="card"
    >
      {children}
    </Component>
  );
}
