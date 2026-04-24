/**
 * AsyncAgentDetailDialog — 异步 Agent 详情对话框。
 *
 * 显示异步 Agent 的名称、任务 ID、状态和输出。
 * visible=false 时返回 null。
 */

import { X, Activity } from 'lucide-react';
import { cn } from '@/lib/utils';
import { BackgroundTaskStatus } from './BackgroundTaskStatus';

export interface AsyncAgentDetailDialogProps {
  visible: boolean;
  agentName: string;
  taskId: string;
  status: string;
  output?: string;
  onClose: () => void;
  className?: string;
}

export function AsyncAgentDetailDialog({
  visible,
  agentName,
  taskId,
  status,
  output,
  onClose,
  className,
}: AsyncAgentDetailDialogProps) {
  if (!visible) return null;

  return (
    <div
      data-testid="async-agent-detail"
      className={cn(
        'fixed inset-0 z-50 flex items-center justify-center bg-black/50',
        className,
      )}
    >
      <div className="w-full max-w-lg rounded-xl bg-white shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-200 px-4 py-3">
          <div className="flex items-center gap-2">
            <Activity className="h-4 w-4 text-blue-500" />
            <h2 className="text-sm font-semibold text-slate-900">{agentName}</h2>
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
            <span className="text-slate-500">任务 ID</span>
            <span className="font-mono text-xs text-slate-700">{taskId}</span>
          </div>
          <div className="flex items-center justify-between text-sm">
            <span className="text-slate-500">状态</span>
            <BackgroundTaskStatus status={status as 'running' | 'completed' | 'failed' | 'pending'} />
          </div>
          {output && (
            <div>
              <span className="text-sm text-slate-500">输出</span>
              <pre className="mt-1 max-h-48 overflow-auto rounded-lg bg-slate-50 p-3 text-xs text-slate-700">
                {output}
              </pre>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
