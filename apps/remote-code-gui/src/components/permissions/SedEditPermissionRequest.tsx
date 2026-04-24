import { FileEdit } from 'lucide-react';
import type { PermissionRequestInfo } from '../../lib/types';
import { cn } from '../../lib/utils';
import { PermissionRequestTitle } from './PermissionRequestTitle';

export interface SedEditPermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function SedEditPermissionRequest({ request, onAllow, onReject, className }: SedEditPermissionRequestProps) {
  return (
    <div className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)} data-testid="sed-edit-permission">
      <PermissionRequestTitle title={request.title || 'Sed 编辑'} subtitle={request.description} />
      <div className="my-3 flex items-center gap-2 rounded-lg bg-slate-50 px-3 py-2">
        <FileEdit className="h-4 w-4 text-slate-400" />
        <span className="text-sm text-slate-600">请求使用 sed 编辑文件</span>
      </div>
      <div className="flex gap-2">
        <button className="rounded-lg bg-blue-600 px-4 py-1.5 text-sm text-white hover:bg-blue-700" onClick={onAllow}>允许执行</button>
        <button className="rounded-lg border border-slate-300 px-4 py-1.5 text-sm text-slate-600 hover:bg-slate-50" onClick={() => onReject()}>拒绝</button>
      </div>
    </div>
  );
}
