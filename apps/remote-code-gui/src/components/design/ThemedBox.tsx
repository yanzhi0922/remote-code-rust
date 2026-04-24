import { cn } from '../../lib/utils';

export interface ThemedBoxProps {
  theme?: 'default' | 'primary' | 'success' | 'warning' | 'error';
  padding?: 'sm' | 'md' | 'lg';
  rounded?: boolean;
  className?: string;
  children: React.ReactNode;
}

const THEME_MAP = {
  default: 'bg-white border-slate-200 text-slate-800',
  primary: 'bg-blue-50 border-blue-200 text-blue-800',
  success: 'bg-green-50 border-green-200 text-green-800',
  warning: 'bg-yellow-50 border-yellow-200 text-yellow-800',
  error: 'bg-red-50 border-red-200 text-red-800',
};

const PADDING_MAP = {
  sm: 'p-2',
  md: 'p-4',
  lg: 'p-6',
};

export function ThemedBox({
  theme = 'default',
  padding = 'md',
  rounded = false,
  className,
  children,
}: ThemedBoxProps) {
  return (
    <div
      data-testid="themed-box"
      className={cn(
        'border',
        THEME_MAP[theme],
        PADDING_MAP[padding],
        rounded ? 'rounded-xl' : 'rounded-lg',
        className
      )}
    >
      {children}
    </div>
  );
}
