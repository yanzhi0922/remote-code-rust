/**
 * WorkflowDetailDialog — 工作流详情对话框。
 *
 * 显示工作流名称和步骤列表，每个步骤带状态指示。
 * visible=false 时返回 null。
 */

import { X, GitBranch, CheckCircle2, XCircle, Clock, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface WorkflowStep {
  name: string;
  status: string;
}

export interface WorkflowDetailDialogProps {
  visible: boolean;
  workflowName: string;
  steps: WorkflowStep[];
  onClose: () => void;
  className?: string;
}

const stepIconMap: Record<string, { icon: typeof CheckCircle2; color: string }> = {
  completed: { icon: CheckCircle2, color: 'text-green-500' },
  failed: { icon: XCircle, color: 'text-red-500' },
  running: { icon: Loader2, color: 'text-blue-500' },
  pending: { icon: Clock, color: 'text-slate-400' },
};

export function WorkflowDetailDialog({
  visible,
  workflowName,
  steps,
  onClose,
  className,
}: WorkflowDetailDialogProps) {
  if (!visible) return null;

  return (
    <div
      data-testid="workflow-detail-dialog"
      className={cn(
        'fixed inset-0 z-50 flex items-center justify-center bg-black/50',
        className,
      )}
    >
      <div className="w-full max-w-lg rounded-xl bg-white shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-200 px-4 py-3">
          <div className="flex items-center gap-2">
            <GitBranch className="h-4 w-4 text-purple-500" />
            <h2 className="text-sm font-semibold text-slate-900">{workflowName}</h2>
          </div>
          <button
            onClick={onClose}
            className="rounded-lg p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
            aria-label="关闭"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Steps list */}
        <div className="max-h-80 overflow-y-auto p-4">
          {steps.length === 0 ? (
            <p className="py-8 text-center text-sm text-slate-400">暂无步骤</p>
          ) : (
            <div className="space-y-2">
              {steps.map((step, index) => {
                const config = stepIconMap[step.status] ?? stepIconMap.pending;
                const StepIcon = config.icon;
                return (
                  <div
                    key={`${step.name}-${index}`}
                    className="flex items-center gap-3 rounded-lg border border-slate-100 px-3 py-2"
                  >
                    <StepIcon
                      className={cn(
                        'h-4 w-4 shrink-0',
                        config.color,
                        step.status === 'running' && 'animate-spin',
                      )}
                    />
                    <span className="flex-1 text-sm text-slate-700">{step.name}</span>
                    <span className="text-xs text-slate-400">{step.status}</span>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
