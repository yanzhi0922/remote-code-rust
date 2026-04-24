import { HelpCircle } from 'lucide-react';
import type { PermissionRequestInfo } from '../../lib/types';
import { cn } from '../../lib/utils';
import { PermissionRequestTitle } from './PermissionRequestTitle';

export interface AskUserQuestionPermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function AskUserQuestionPermissionRequest({
  request,
  onAllow,
  onReject,
  className,
}: AskUserQuestionPermissionRequestProps) {
  return (
    <div
      className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)}
      data-testid="ask-user-question-permission"
    >
      <PermissionRequestTitle title={request.title || '用户问题'} subtitle={request.description} />
      <div className="my-3 flex items-start gap-2 rounded-lg bg-slate-50 px-3 py-2">
        <HelpCircle className="mt-0.5 h-4 w-4 shrink-0 text-slate-400" />
        <p className="text-sm text-slate-700">{request.description}</p>
      </div>
      <div className="flex gap-2">
        <button
          className="rounded-lg bg-blue-600 px-4 py-1.5 text-sm text-white hover:bg-blue-700"
          onClick={onAllow}
        >
          回答
        </button>
        <button
          className="rounded-lg border border-slate-300 px-4 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
          onClick={() => onReject()}
        >
          跳过
        </button>
      </div>
    </div>
  );
}
