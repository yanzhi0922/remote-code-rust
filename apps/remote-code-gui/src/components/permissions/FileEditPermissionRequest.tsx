import { FileEdit } from 'lucide-react';
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

export interface FileEditPermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  showDiff?: boolean;
  className?: string;
}

export function FileEditPermissionRequest({
  request,
  onAllow,
  onReject,
  showDiff = true,
  className,
}: FileEditPermissionRequestProps) {
  const record = asRecord(request.input);
  const filePath = stringField(record, 'file_path', 'path') ?? '';
  const oldString = stringField(record, 'old_string', 'old_text') ?? '';
  const newString = stringField(record, 'new_string', 'new_text') ?? '';

  return (
    <div
      className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)}
      data-testid="file-edit-permission-request"
    >
      <PermissionRequestTitle
        title={request.title || 'File Edit'}
        subtitle={request.description}
      />

      {filePath && (
        <div className="my-2 flex items-center gap-2 rounded-lg bg-slate-50 px-3 py-2 font-mono text-sm text-slate-700">
          <FileEdit size={14} className="shrink-0 text-slate-400" />
          <span className="truncate">{filePath}</span>
        </div>
      )}

      {showDiff && (
        <div className="my-2 space-y-2">
          {oldString && (
            <div className="rounded-lg bg-red-50 p-2 font-mono text-xs text-red-800">
              <div className="mb-1 text-xs font-semibold text-red-600">- 删除内容</div>
              <pre className="whitespace-pre-wrap break-all">{oldString}</pre>
            </div>
          )}
          {newString && (
            <div className="rounded-lg bg-emerald-50 p-2 font-mono text-xs text-emerald-800">
              <div className="mb-1 text-xs font-semibold text-emerald-600">+ 新增内容</div>
              <pre className="whitespace-pre-wrap break-all">{newString}</pre>
            </div>
          )}
          {!oldString && !newString && (
            <div className="rounded-lg bg-slate-50 p-2 text-xs text-slate-400">
              无 diff 内容
            </div>
          )}
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
