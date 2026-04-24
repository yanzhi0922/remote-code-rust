/**
 * ShellProgress — Shell 命令进度显示组件。
 *
 * 显示正在执行的命令和已用时间。
 */

import { Terminal, Clock } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface ShellProgressProps {
  command: string;
  elapsedTime: number;
  className?: string;
}

function formatElapsed(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes}m ${remainingSeconds}s`;
}

export function ShellProgress({
  command,
  elapsedTime,
  className,
}: ShellProgressProps) {
  return (
    <div
      data-testid="shell-progress"
      className={cn(
        'flex items-center gap-3 rounded-lg border border-slate-200 bg-white px-3 py-2',
        className,
      )}
    >
      <Terminal className="h-4 w-4 shrink-0 text-slate-500" />
      <span className="flex-1 truncate font-mono text-xs text-slate-700">
        {command}
      </span>
      <span className="flex items-center gap-1 text-xs text-slate-400">
        <Clock className="h-3 w-3" />
        {formatElapsed(elapsedTime)}
      </span>
    </div>
  );
}
