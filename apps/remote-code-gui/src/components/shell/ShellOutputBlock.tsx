/**
 * ShellOutputBlock — Shell 命令输出块组件。
 *
 * 显示命令行输出块，带命令标题、执行时间和退出码状态。
 */

import { Terminal, Clock } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface ShellOutputBlockProps {
  command: string;
  output: string;
  exitCode?: number;
  duration?: number;
  stream?: 'stdout' | 'stderr';
  className?: string;
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes}m ${remainingSeconds}s`;
}

export function ShellOutputBlock({
  command,
  output,
  exitCode,
  duration,
  stream = 'stdout',
  className,
}: ShellOutputBlockProps) {
  const isError = exitCode !== undefined && exitCode !== 0;

  return (
    <div
      data-testid="shell-output-block"
      className={cn(
        'rounded-lg border bg-slate-950 text-slate-100 font-mono text-sm',
        isError ? 'border-red-500' : 'border-slate-700',
        className,
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between border-b border-slate-700 px-3 py-2">
        <div className="flex items-center gap-2 text-slate-300">
          <Terminal className="h-4 w-4" />
          <span className="truncate text-xs">{command}</span>
        </div>
        <div className="flex items-center gap-3 text-xs text-slate-400">
          {stream === 'stderr' && (
            <span className="text-red-400">stderr</span>
          )}
          {duration !== undefined && (
            <span className="flex items-center gap-1">
              <Clock className="h-3 w-3" />
              {formatDuration(duration)}
            </span>
          )}
          {exitCode !== undefined && (
            <span className={isError ? 'text-red-400' : 'text-green-400'}>
              exit: {exitCode}
            </span>
          )}
        </div>
      </div>

      {/* Output body */}
      <pre className="max-h-80 overflow-auto whitespace-pre-wrap p-3 text-xs leading-5">
        {output}
      </pre>
    </div>
  );
}
