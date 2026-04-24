/**
 * BackgroundTask — 后台任务卡片组件。
 *
 * 显示任务名称、状态徽章和进度条。
 * running 状态带绿色脉冲动画，failed 红色，completed 绿色 ✓。
 */

import { CheckCircle2, XCircle, Clock, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface TaskData {
  id: string;
  name: string;
  status: 'running' | 'completed' | 'failed' | 'pending';
  progress?: number;
  startedAt: string;
}

export interface BackgroundTaskProps {
  task: TaskData;
  onClick?: () => void;
  className?: string;
}

const statusConfig: Record<
  TaskData['status'],
  { icon: typeof CheckCircle2; colorClass: string; label: string }
> = {
  running: { icon: Loader2, colorClass: 'text-green-500', label: 'Running' },
  completed: { icon: CheckCircle2, colorClass: 'text-green-500', label: 'Completed' },
  failed: { icon: XCircle, colorClass: 'text-red-500', label: 'Failed' },
  pending: { icon: Clock, colorClass: 'text-slate-400', label: 'Pending' },
};

export function BackgroundTask({ task, onClick, className }: BackgroundTaskProps) {
  const config = statusConfig[task.status];
  const StatusIcon = config.icon;

  return (
    <div
      data-testid="background-task"
      className={cn(
        'rounded-lg border border-slate-200 bg-white p-3 transition-shadow hover:shadow-md',
        task.status === 'running' && 'border-green-300 shadow-sm',
        task.status === 'failed' && 'border-red-300',
        onClick && 'cursor-pointer',
        className,
      )}
      onClick={onClick}
      role={onClick ? 'button' : undefined}
      tabIndex={onClick ? 0 : undefined}
    >
      <div className="flex items-center justify-between">
        <span className="truncate text-sm font-medium text-slate-900">
          {task.name}
        </span>
        <div className={cn('flex items-center gap-1 text-xs', config.colorClass)}>
          <StatusIcon
            className={cn(
              'h-3.5 w-3.5',
              task.status === 'running' && 'animate-spin',
              task.status === 'running' && 'animate-pulse',
            )}
          />
          <span>{config.label}</span>
        </div>
      </div>

      {/* Progress bar */}
      {task.progress !== undefined && (
        <div className="mt-2 h-1.5 w-full rounded-full bg-slate-100">
          <div
            className={cn(
              'h-1.5 rounded-full transition-all duration-300',
              task.status === 'failed' ? 'bg-red-500' : 'bg-green-500',
            )}
            style={{ width: `${Math.min(task.progress, 100)}%` }}
          />
        </div>
      )}

      <div className="mt-1 text-xs text-slate-400">
        {task.startedAt}
      </div>
    </div>
  );
}
