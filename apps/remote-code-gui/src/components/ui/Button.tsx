/**
 * Button — 通用按钮组件。
 *
 * 支持 primary / secondary / ghost / danger 四种变体，
 * 三种尺寸，加载态、禁用态和左侧图标。
 */

import { type ReactNode, type ButtonHTMLAttributes } from 'react';
import { Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  size?: 'sm' | 'md' | 'lg';
  loading?: boolean;
  icon?: ReactNode;
  children: ReactNode;
}

const variantStyles: Record<NonNullable<ButtonProps['variant']>, string> = {
  primary:
    'bg-slate-800 text-white hover:bg-slate-700 active:bg-slate-900 focus-visible:ring-slate-500',
  secondary:
    'border border-slate-300 text-slate-700 bg-white hover:bg-slate-50 active:bg-slate-100 focus-visible:ring-slate-400',
  ghost:
    'text-slate-600 hover:bg-slate-100 active:bg-slate-200 focus-visible:ring-slate-400',
  danger:
    'bg-red-600 text-white hover:bg-red-500 active:bg-red-700 focus-visible:ring-red-400',
};

const sizeStyles: Record<NonNullable<ButtonProps['size']>, string> = {
  sm: 'h-8 px-3 text-sm gap-1.5 rounded-lg',
  md: 'h-10 px-4 text-sm gap-2 rounded-xl',
  lg: 'h-12 px-6 text-base gap-2.5 rounded-xl',
};

export function Button({
  variant = 'primary',
  size = 'md',
  disabled = false,
  loading = false,
  icon,
  children,
  className,
  type = 'button',
  onClick,
  ...rest
}: ButtonProps) {
  const isDisabled = disabled || loading;

  return (
    <button
      type={type}
      disabled={isDisabled}
      onClick={isDisabled ? undefined : onClick}
      className={cn(
        'inline-flex items-center justify-center font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2',
        variantStyles[variant],
        sizeStyles[size],
        isDisabled && 'pointer-events-none opacity-50',
        className,
      )}
      data-testid="button"
      {...rest}
    >
      {loading ? (
        <Loader2 className="h-4 w-4 animate-spin" data-testid="button-spinner" />
      ) : icon ? (
        <span className="flex-shrink-0" data-testid="button-icon">
          {icon}
        </span>
      ) : null}
      {children}
    </button>
  );
}
