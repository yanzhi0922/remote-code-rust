/**
 * BackgroundTaskStatus — 任务状态徽章组件。
 *
 * 根据状态显示不同颜色和图标。
 */

import { CheckCircle2, XCircle, Clock, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface BackgroundTaskStatusProps {
  status: 'running' | 'completed' | 'failed' | 'pending';
  className?: string;
}

const statusMap: Record<
  BackgroundTaskStatusProps['status'],
  { icon: typeof CheckCircle2; colorClass: string; bgClass: string; label: string }
> = {
  running: {
    icon: Loader2,
    colorClass: 'text-green-700',
    bgClass: 'bg-green-50',
    label: 'Running',
  },
  completed: {
    icon: CheckCircle2,
    colorClass: 'text-green-700',
    bgClass: 'bg-green-50',
    label: 'Completed',
  },
  failed: {
    icon: XCircle,
    colorClass: 'text-red-700',
    bgClass: 'bg-red-50',
    label: 'Failed',
  },
  pending: {
    icon: Clock,
    colorClass: 'text-slate-600',
    bgClass: 'bg-slate-100',
    label: 'Pending',
  },
};

export function BackgroundTaskStatus({ status, className }: BackgroundTaskStatusProps) {
  const config = statusMap[status];
  const StatusIcon = config.icon;

  return (
    <span
      data-testid="background-task-status"
      className={cn(
        'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium',
        config.bgClass,
        config.colorClass,
        className,
      )}
    >
      <StatusIcon
        className={cn(
          'h-3 w-3',
          status === 'running' && 'animate-spin',
        )}
      />
      {config.label}
    </span>
  );
}
