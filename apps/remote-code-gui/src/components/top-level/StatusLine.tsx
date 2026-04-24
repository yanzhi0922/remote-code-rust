import { Activity } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface StatusLineProps {
  status: 'idle' | 'running' | 'error' | 'success';
  message?: string;
}

export function StatusLine({ status, message }: StatusLineProps) {
  const colors: Record<string, string> = {
    idle: 'text-slate-400',
    running: 'text-blue-500',
    error: 'text-red-500',
    success: 'text-green-500',
  };

  return (
    <div data-testid="status-line" className="flex items-center gap-2 text-xs">
      <Activity className={cn('h-3.5 w-3.5', colors[status])} />
      <span className={colors[status]}>
        {message ?? (status === 'idle' ? '空闲' : status === 'running' ? '运行中' : status === 'error' ? '错误' : '完成')}
      </span>
    </div>
  );
}
