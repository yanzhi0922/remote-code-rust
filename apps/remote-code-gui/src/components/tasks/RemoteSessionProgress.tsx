/**
 * RemoteSessionProgress — 远程会话传输进度组件。
 *
 * 显示服务器 URL 和传输字节数进度条。
 */

import { Shield } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface RemoteSessionProgressProps {
  serverUrl: string;
  bytesTransferred?: number;
  totalBytes?: number;
  className?: string;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function RemoteSessionProgress({
  serverUrl,
  bytesTransferred,
  totalBytes,
  className,
}: RemoteSessionProgressProps) {
  const hasProgress =
    bytesTransferred !== undefined &&
    totalBytes !== undefined &&
    totalBytes > 0;
  const pct = hasProgress
    ? Math.min((bytesTransferred! / totalBytes!) * 100, 100)
    : 0;

  return (
    <div
      data-testid="remote-session-progress"
      className={cn(
        'rounded-lg border border-slate-200 bg-white p-3',
        className,
      )}
    >
      <div className="flex items-center gap-2">
        <Shield className="h-4 w-4 shrink-0 text-blue-500" />
        <span className="truncate font-mono text-xs text-slate-700">
          {serverUrl}
        </span>
      </div>

      {hasProgress && (
        <div className="mt-2">
          <div className="mb-1 flex items-center justify-between text-xs text-slate-400">
            <span>{formatBytes(bytesTransferred!)}</span>
            <span>{Math.round(pct)}%</span>
          </div>
          <div className="h-1.5 w-full rounded-full bg-slate-100">
            <div
              className="h-1.5 rounded-full bg-blue-500 transition-all duration-300"
              style={{ width: `${pct}%` }}
            />
          </div>
        </div>
      )}
    </div>
  );
}
