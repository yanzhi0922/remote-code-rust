import { BookOpen } from 'lucide-react';
import type { PermissionRequestInfo } from '../../lib/types';
import { cn } from '../../lib/utils';
import { PermissionRequestTitle } from './PermissionRequestTitle';

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */

export interface EnterPlanModePermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function EnterPlanModePermissionRequest({
  request,
  onAllow,
  onReject,
  className,
}: EnterPlanModePermissionRequestProps) {
  return (
    <div
      className={cn('rounded-2xl border-2 border-blue-300 bg-blue-50 p-4', className)}
      data-testid="enter-plan-mode-request"
    >
      <div className="flex items-center gap-2">
        <BookOpen size={18} className="text-blue-500" />
        <PermissionRequestTitle
          title={request.title || '进入计划模式'}
          subtitle={request.description}
          color="#3b82f6"
        />
      </div>

      <div className="my-3 rounded-lg bg-white px-3 py-2 text-sm text-blue-700">
        请求进入计划模式。在此模式下，助手将仅规划任务而不执行任何操作。
      </div>

      <div className="mt-3 flex gap-2">
        <button
          type="button"
          onClick={onAllow}
          className="rounded-2xl bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700"
        >
          允许
        </button>
        <button
          type="button"
          onClick={() => onReject()}
          className="rounded-2xl border border-blue-300 bg-white px-4 py-2 text-sm font-medium text-blue-600 transition-colors hover:bg-blue-50"
        >
          拒绝
        </button>
      </div>
    </div>
  );
}
