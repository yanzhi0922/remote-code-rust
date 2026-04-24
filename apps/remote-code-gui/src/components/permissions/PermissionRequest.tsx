import type { PermissionRequestInfo } from '../../lib/types';
import { cn } from '../../lib/utils';
import { PermissionRequestTitle } from './PermissionRequestTitle';

export interface PermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function PermissionRequest({ request, onAllow, onReject, className }: PermissionRequestProps) {
  return (
    <div className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)} data-testid="permission-request">
      <PermissionRequestTitle title={request.title || '权限请求'} subtitle={request.description} />
      <p className="mt-2 text-xs text-slate-400">工具: {request.tool_name}</p>
      <div className="mt-3 flex gap-2">
        <button className="rounded-lg bg-blue-600 px-4 py-1.5 text-sm text-white hover:bg-blue-700" onClick={onAllow}>允许执行</button>
        <button className="rounded-lg border border-slate-300 px-4 py-1.5 text-sm text-slate-600 hover:bg-slate-50" onClick={() => onReject()}>拒绝</button>
      </div>
    </div>
  );
}
