import { Shield } from 'lucide-react';
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

export interface SkillPermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function SkillPermissionRequest({
  request,
  onAllow,
  onReject,
  className,
}: SkillPermissionRequestProps) {
  const record = asRecord(request.input);
  const skillName = stringField(record, 'skill_name', 'skill', 'name') ?? '';

  return (
    <div
      className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)}
      data-testid="skill-permission-request"
    >
      <PermissionRequestTitle
        title={request.title || 'Skill Execution'}
        subtitle={request.description}
      />

      {skillName ? (
        <div className="my-3 flex items-center gap-2 rounded-lg bg-slate-50 px-3 py-2">
          <Shield size={14} className="shrink-0 text-slate-400" />
          <span className="text-sm text-slate-500">技能名称:</span>
          <span className="font-mono text-sm font-medium text-slate-800">{skillName}</span>
        </div>
      ) : (
        <div className="my-3 rounded-lg bg-slate-50 p-2 text-sm text-slate-400">
          无技能名称
        </div>
      )}

      <div className="mt-3 flex gap-2">
        <button
          type="button"
          onClick={onAllow}
          className="rounded-2xl bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700"
        >
          允许执行
        </button>
        <button
          type="button"
          onClick={() => onReject()}
          className="rounded-2xl border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-50"
        >
          拒绝
        </button>
      </div>
    </div>
  );
}
