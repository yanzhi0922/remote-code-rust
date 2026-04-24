import { FileText } from 'lucide-react';
import type { PermissionRequestInfo } from '../../lib/types';
import { cn } from '../../lib/utils';
import { PermissionRequestTitle } from './PermissionRequestTitle';

export interface FilePermissionDialogProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function FilePermissionDialog({
  request,
  onAllow,
  onReject,
  className,
}: FilePermissionDialogProps) {
  const input = request.input as Record<string, unknown> | null;
  const filePath = (input?.path ?? input?.file_path ?? '') as string;

  return (
    <div
      className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)}
      data-testid="file-permission-dialog"
    >
      <PermissionRequestTitle title={request.title || '文件权限'} subtitle={request.description} />
      {filePath && (
        <div className="my-3 flex items-center gap-2 rounded-lg bg-slate-50 px-3 py-2 font-mono text-sm">
          <FileText className="h-4 w-4 shrink-0 text-slate-400" />
          <span className="truncate">{filePath}</span>
        </div>
      )}
      <div className="flex gap-2">
        <button className="rounded-lg bg-blue-600 px-4 py-1.5 text-sm text-white hover:bg-blue-700" onClick={onAllow}>
          允许执行
        </button>
        <button className="rounded-lg border border-slate-300 px-4 py-1.5 text-sm text-slate-600 hover:bg-slate-50" onClick={() => onReject()}>
          拒绝
        </button>
      </div>
    </div>
  );
}
