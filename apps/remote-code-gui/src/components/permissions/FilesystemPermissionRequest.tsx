import { FolderOpen } from 'lucide-react';
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

const OPERATION_LABELS: Record<string, string> = {
  read: '读取',
  write: '写入',
  list: '列出目录',
  delete: '删除',
  rename: '重命名',
  move: '移动',
  copy: '复制',
};

function getOperationLabel(operation: string): string {
  return OPERATION_LABELS[operation.toLowerCase()] ?? operation;
}

function getOperationColor(operation: string): string {
  switch (operation.toLowerCase()) {
    case 'read':
    case 'list':
      return 'bg-blue-50 text-blue-700';
    case 'write':
    case 'delete':
    case 'rename':
    case 'move':
      return 'bg-amber-50 text-amber-700';
    default:
      return 'bg-slate-50 text-slate-700';
  }
}

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */

export interface FilesystemPermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function FilesystemPermissionRequest({
  request,
  onAllow,
  onReject,
  className,
}: FilesystemPermissionRequestProps) {
  const record = asRecord(request.input);
  const path = stringField(record, 'path', 'file_path') ?? '';
  const operation = stringField(record, 'operation', 'action') ?? '';

  return (
    <div
      className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)}
      data-testid="filesystem-permission-request"
    >
      <PermissionRequestTitle
        title={request.title || 'Filesystem Access'}
        subtitle={request.description}
      />

      <div className="my-3 space-y-2">
        {path && (
          <div className="flex items-center gap-2 rounded-lg bg-slate-50 px-3 py-2 font-mono text-sm text-slate-700">
            <FolderOpen size={14} className="shrink-0 text-slate-400" />
            <span className="truncate">{path}</span>
          </div>
        )}
        {operation && (
          <div className="flex items-center gap-2">
            <span className="text-xs text-slate-500">操作类型:</span>
            <span
              className={cn(
                'inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium',
                getOperationColor(operation),
              )}
            >
              {getOperationLabel(operation)}
            </span>
          </div>
        )}
        {!path && !operation && (
          <div className="rounded-lg bg-slate-50 p-2 text-xs text-slate-400">
            无路径或操作信息
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
