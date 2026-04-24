import { FileText } from 'lucide-react';
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

const MAX_CONTENT_PREVIEW = 500;

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */

export interface FileWritePermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function FileWritePermissionRequest({
  request,
  onAllow,
  onReject,
  className,
}: FileWritePermissionRequestProps) {
  const record = asRecord(request.input);
  const filePath = stringField(record, 'file_path', 'path') ?? '';
  const rawContent = stringField(record, 'content') ?? '';
  const truncated = rawContent.length > MAX_CONTENT_PREVIEW;
  const contentPreview = truncated
    ? rawContent.slice(0, MAX_CONTENT_PREVIEW) + '…'
    : rawContent;

  return (
    <div
      className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)}
      data-testid="file-write-permission-request"
    >
      <PermissionRequestTitle
        title={request.title || 'File Write'}
        subtitle={request.description}
      />

      {filePath && (
        <div className="my-2 flex items-center gap-2 rounded-lg bg-slate-50 px-3 py-2 font-mono text-sm text-slate-700">
          <FileText size={14} className="shrink-0 text-slate-400" />
          <span className="truncate">{filePath}</span>
        </div>
      )}

      {contentPreview ? (
        <div className="my-2 max-h-40 overflow-y-auto rounded-lg bg-slate-50 p-2 font-mono text-xs text-slate-700">
          <pre className="whitespace-pre-wrap break-all">{contentPreview}</pre>
          {truncated && (
            <div className="mt-1 text-xs text-slate-400">
              内容已截断（共 {rawContent.length} 字符）
            </div>
          )}
        </div>
      ) : (
        <div className="my-2 rounded-lg bg-slate-50 p-2 text-xs text-slate-400">
          无文件内容
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
