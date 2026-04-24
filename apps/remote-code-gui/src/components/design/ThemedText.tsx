import { cn } from '../../lib/utils';

export interface ThemedTextProps {
  theme?: 'default' | 'primary' | 'success' | 'warning' | 'error' | 'muted';
  size?: 'xs' | 'sm' | 'md' | 'lg';
  bold?: boolean;
  className?: string;
  children: React.ReactNode;
}

const THEME_MAP = {
  default: 'text-slate-800',
  primary: 'text-blue-600',
  success: 'text-green-600',
  warning: 'text-yellow-600',
  error: 'text-red-600',
  muted: 'text-slate-400',
};

const SIZE_MAP = {
  xs: 'text-xs',
  sm: 'text-sm',
  md: 'text-base',
  lg: 'text-lg',
};

export function ThemedText({
  theme = 'default',
  size = 'md',
  bold = false,
  className,
  children,
}: ThemedTextProps) {
  return (
    <span
      data-testid="themed-text"
      className={cn(
        THEME_MAP[theme],
        SIZE_MAP[size],
        bold && 'font-bold',
        className
      )}
    >
      {children}
    </span>
  );
}
