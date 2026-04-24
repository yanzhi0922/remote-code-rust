/**
 * RemoteSessionDetailDialog — 远程会话详情对话框。
 *
 * 显示会话 ID、服务器 URL 和状态。
 * visible=false 时返回 null。
 */

import { X, Zap } from 'lucide-react';
import { cn } from '@/lib/utils';
import { BackgroundTaskStatus } from './BackgroundTaskStatus';

export interface RemoteSessionDetailDialogProps {
  visible: boolean;
  sessionId: string;
  serverUrl: string;
  status: string;
  onClose: () => void;
  className?: string;
}

export function RemoteSessionDetailDialog({
  visible,
  sessionId,
  serverUrl,
  status,
  onClose,
  className,
}: RemoteSessionDetailDialogProps) {
  if (!visible) return null;

  return (
    <div
      data-testid="remote-session-detail"
      className={cn(
        'fixed inset-0 z-50 flex items-center justify-center bg-black/50',
        className,
      )}
    >
      <div className="w-full max-w-lg rounded-xl bg-white shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-200 px-4 py-3">
          <div className="flex items-center gap-2">
            <Zap className="h-4 w-4 text-amber-500" />
            <h2 className="text-sm font-semibold text-slate-900">远程会话详情</h2>
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
          <div className="flex items-center justify-between text-sm">
            <span className="text-slate-500">会话 ID</span>
            <span className="font-mono text-xs text-slate-700">{sessionId}</span>
          </div>
          <div className="flex items-center justify-between text-sm">
            <span className="text-slate-500">服务器</span>
            <span className="font-mono text-xs text-slate-700">{serverUrl}</span>
          </div>
          <div className="flex items-center justify-between text-sm">
            <span className="text-slate-500">状态</span>
            <BackgroundTaskStatus status={status as 'running' | 'completed' | 'failed' | 'pending'} />
          </div>
        </div>
      </div>
    </div>
  );
}
