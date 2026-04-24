/**
 * ShellProgressMessage — Shell 命令进度消息组件。
 *
 * 显示正在执行的命令，带进度条或 spinner。
 */

import { Loader2, Play } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface ShellProgressMessageProps {
  command: string;
  progress?: number;
  total?: number;
  className?: string;
}

export function ShellProgressMessage({
  command,
  progress,
  total,
  className,
}: ShellProgressMessageProps) {
  const hasProgress = progress !== undefined && total !== undefined && total > 0;
  const pct = hasProgress ? Math.min((progress! / total!) * 100, 100) : 0;

  return (
    <div
      data-testid="shell-progress-message"
      className={cn('flex items-center gap-3 rounded-lg bg-slate-900 px-3 py-2', className)}
    >
      {hasProgress ? (
        <Play className="h-4 w-4 shrink-0 text-blue-400" />
      ) : (
        <Loader2 className="h-4 w-4 shrink-0 animate-spin text-blue-400" />
      )}

      <div className="flex-1 min-w-0">
        <p className="truncate font-mono text-xs text-slate-300">{command}</p>
        {hasProgress && (
          <div className="mt-1 h-1.5 w-full rounded-full bg-slate-700">
            <div
              className="h-1.5 rounded-full bg-blue-500 transition-all duration-300"
              style={{ width: `${pct}%` }}
            />
          </div>
        )}
      </div>

      {hasProgress && (
        <span className="shrink-0 text-xs text-slate-400">
          {Math.round(pct)}%
        </span>
      )}
    </div>
  );
}
