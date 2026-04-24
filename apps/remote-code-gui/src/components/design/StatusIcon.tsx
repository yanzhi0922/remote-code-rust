import { CheckCircle2, XCircle, AlertTriangle, Info, Loader2 } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface StatusIconProps {
  status: 'success' | 'error' | 'warning' | 'info' | 'loading';
  className?: string;
}

const STATUS_CONFIG = {
  success: { icon: CheckCircle2, color: 'text-green-500' },
  error: { icon: XCircle, color: 'text-red-500' },
  warning: { icon: AlertTriangle, color: 'text-yellow-500' },
  info: { icon: Info, color: 'text-blue-500' },
  loading: { icon: Loader2, color: 'text-blue-500 animate-spin' },
} as const;

export function StatusIcon({ status, className }: StatusIconProps) {
  const config = STATUS_CONFIG[status];
  const Icon = config.icon;

  return (
    <span data-testid="status-icon" className={cn('inline-flex', className)}>
      <Icon
        data-testid={`status-icon-${status}`}
        className={cn('h-5 w-5', config.color)}
      />
    </span>
  );
}
