import { Loader2 } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface LoadingStateProps {
  message?: string;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

const SIZE_MAP = {
  sm: 'h-4 w-4',
  md: 'h-6 w-6',
  lg: 'h-10 w-10',
};

const TEXT_SIZE_MAP = {
  sm: 'text-xs',
  md: 'text-sm',
  lg: 'text-base',
};

export function LoadingState({ message, size = 'md', className }: LoadingStateProps) {
  return (
    <div data-testid="loading-state" className={cn('flex flex-col items-center justify-center gap-2', className)}>
      <Loader2
        data-testid="loading-spinner"
        className={cn('animate-spin text-blue-600', SIZE_MAP[size])}
      />
      {message && (
        <span data-testid="loading-message" className={cn('text-slate-500', TEXT_SIZE_MAP[size])}>
          {message}
        </span>
      )}
    </div>
  );
}
