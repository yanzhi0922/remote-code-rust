import { CheckCircle2 } from 'lucide-react';
import type { PermissionRequestInfo } from '../../lib/types';
import { cn } from '../../lib/utils';
import { PermissionRequestTitle } from './PermissionRequestTitle';

/* ------------------------------------------------------------------ */
/* Helpers                                                             */
/* ------------------------------------------------------------------ */

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function stringField(record: Record<string, unknown> | null, ...keys: string[]): string | null {
  if (!record) return null;
  for (const key of keys) {
    const val = record[key];
    if (typeof val === 'string' && val.trim()) return val;
  }
  return null;
}

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */

export interface ExitPlanModePermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function ExitPlanModePermissionRequest({
  request,
  onAllow,
  onReject,
  className,
}: ExitPlanModePermissionRequestProps) {
  const record = asRecord(request.input);
  const planSummary = stringField(record, 'plan', 'plan_summary', 'summary', 'content') ?? '';

  return (
    <div
      className={cn('rounded-2xl border-2 border-emerald-300 bg-emerald-50 p-4', className)}
      data-testid="exit-plan-mode-request"
    >
      <div className="flex items-center gap-2">
        <CheckCircle2 size={18} className="text-emerald-500" />
        <PermissionRequestTitle
          title={request.title || '退出计划模式'}
          subtitle={request.description}
          color="#10b981"
        />
      </div>

      {planSummary ? (
        <div className="my-3 max-h-40 overflow-y-auto rounded-lg bg-white p-3 text-sm text-emerald-800">
          <div className="mb-1 text-xs font-semibold text-emerald-600">计划摘要:</div>
          <pre className="whitespace-pre-wrap break-all">{planSummary}</pre>
        </div>
      ) : (
        <div className="my-3 rounded-lg bg-white p-3 text-sm text-emerald-600">
          请求退出计划模式并开始执行任务。
        </div>
      )}

      <div className="mt-3 flex gap-2">
        <button
          type="button"
          onClick={onAllow}
          className="rounded-2xl bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-emerald-700"
        >
          允许
        </button>
        <button
          type="button"
          onClick={() => onReject()}
          className="rounded-2xl border border-emerald-300 bg-white px-4 py-2 text-sm font-medium text-emerald-600 transition-colors hover:bg-emerald-50"
        >
          拒绝
        </button>
      </div>
    </div>
  );
}
