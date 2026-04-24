import { cn } from '../../lib/utils';

export interface WorkerBadgeProps {
  name: string;
  status?: 'idle' | 'busy' | 'error';
  className?: string;
}

export function WorkerBadge({ name, status = 'idle', className }: WorkerBadgeProps) {
  const colors: Record<string, string> = {
    idle: 'bg-slate-100 text-slate-600',
    busy: 'bg-blue-100 text-blue-700',
    error: 'bg-red-100 text-red-700',
  };

  return (
    <span className={cn('inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium', colors[status], className)} data-testid="worker-badge">
      {name}
    </span>
  );
}
