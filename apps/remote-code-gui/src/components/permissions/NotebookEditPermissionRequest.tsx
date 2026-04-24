import { BookOpen } from 'lucide-react';
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

function numberField(record: Record<string, unknown> | null, ...keys: string[]): number | null {
  if (!record) return null;
  for (const key of keys) {
    const val = record[key];
    if (typeof val === 'number') return val;
    if (typeof val === 'string' && /^\d+$/.test(val.trim())) return Number(val);
  }
  return null;
}

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */

export interface NotebookEditPermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function NotebookEditPermissionRequest({
  request,
  onAllow,
  onReject,
  className,
}: NotebookEditPermissionRequestProps) {
  const record = asRecord(request.input);
  const notebookPath = stringField(record, 'notebook_path', 'path', 'file_path') ?? '';
  const cellNumber = numberField(record, 'cell_number', 'cell_index', 'cell');

  return (
    <div
      className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)}
      data-testid="notebook-edit-permission-request"
    >
      <PermissionRequestTitle
        title={request.title || 'Notebook Edit'}
        subtitle={request.description}
      />

      <div className="my-3 space-y-2">
        {notebookPath && (
          <div className="flex items-center gap-2 rounded-lg bg-slate-50 px-3 py-2 font-mono text-sm text-slate-700">
            <BookOpen size={14} className="shrink-0 text-slate-400" />
            <span className="truncate">{notebookPath}</span>
          </div>
        )}
        {cellNumber !== null && (
          <div className="flex items-center gap-2">
            <span className="text-xs text-slate-500">Cell 编号:</span>
            <span className="inline-flex items-center rounded-full bg-purple-50 px-2 py-0.5 text-xs font-medium text-purple-700">
              #{cellNumber}
            </span>
          </div>
        )}
        {!notebookPath && cellNumber === null && (
          <div className="rounded-lg bg-slate-50 p-2 text-xs text-slate-400">
            无 Notebook 信息
          </div>
        )}
      </div>

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
