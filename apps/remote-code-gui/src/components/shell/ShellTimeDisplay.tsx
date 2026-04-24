/**
 * ShellTimeDisplay — Shell 执行时间显示组件。
 *
 * 将毫秒时间戳差转换为可读格式，endTime 未提供时显示 "running..."。
 */

import { Clock, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface ShellTimeDisplayProps {
  startTime: number;
  endTime?: number;
  className?: string;
}

function formatMs(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) return `${minutes}m ${remainingSeconds}s`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h ${remainingMinutes}m`;
}

export function ShellTimeDisplay({
  startTime,
  endTime,
  className,
}: ShellTimeDisplayProps) {
  const isRunning = endTime === undefined;

  if (isRunning) {
    return (
      <span
        data-testid="shell-time-display"
        className={cn('inline-flex items-center gap-1 text-xs text-blue-400', className)}
      >
        <Loader2 className="h-3 w-3 animate-spin" />
        running...
      </span>
    );
  }

  const elapsed = endTime - startTime;

  return (
    <span
      data-testid="shell-time-display"
      className={cn('inline-flex items-center gap-1 text-xs text-slate-400', className)}
    >
      <Clock className="h-3 w-3" />
      {formatMs(elapsed)}
    </span>
  );
}
