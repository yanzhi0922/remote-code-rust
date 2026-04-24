/**
 * ShellDetailDialog — Shell 命令详情对话框。
 *
 * 显示命令、输出、退出码和执行时间。
 * visible=false 时返回 null。
 */

import { X, Terminal } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface ShellDetailDialogProps {
  visible: boolean;
  command: string;
  output: string;
  exitCode?: number;
  duration?: number;
  onClose: () => void;
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

export function ShellDetailDialog({
  visible,
  command,
  output,
  exitCode,
  duration,
  onClose,
  className,
}: ShellDetailDialogProps) {
  if (!visible) return null;

  const isError = exitCode !== undefined && exitCode !== 0;

  return (
    <div
      data-testid="shell-detail-dialog"
      className={cn(
        'fixed inset-0 z-50 flex items-center justify-center bg-black/50',
        className,
      )}
    >
      <div className="w-full max-w-xl rounded-xl bg-white shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-200 px-4 py-3">
          <div className="flex items-center gap-2">
            <Terminal className="h-4 w-4 text-slate-500" />
            <h2 className="text-sm font-semibold text-slate-900">Shell 详情</h2>
          </div>
          <button
            onClick={onClose}
            className="rounded-lg p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
            aria-label="关闭"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Body */}
        <div className="space-y-3 p-4">
          <div>
            <span className="text-xs text-slate-500">命令</span>
            <pre className="mt-1 rounded-lg bg-slate-900 p-3 text-xs text-green-400">
              {command}
            </pre>
          </div>

          <div className="flex items-center gap-4 text-sm">
            {exitCode !== undefined && (
              <span className={isError ? 'text-red-600' : 'text-green-600'}>
                退出码: {exitCode}
              </span>
            )}
            {duration !== undefined && (
              <span className="text-slate-500">
                耗时: {formatDuration(duration)}
              </span>
            )}
          </div>

          <div>
            <span className="text-xs text-slate-500">输出</span>
            <pre className="mt-1 max-h-64 overflow-auto rounded-lg bg-slate-50 p-3 text-xs text-slate-700">
              {output}
            </pre>
          </div>
        </div>
      </div>
    </div>
  );
}
