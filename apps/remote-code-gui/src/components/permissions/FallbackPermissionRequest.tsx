import { ShieldAlert } from 'lucide-react';
import type { PermissionRequestInfo } from '../../lib/types';
import { cn } from '../../lib/utils';
import { PermissionRequestTitle } from './PermissionRequestTitle';

export interface FallbackPermissionRequestProps {
  request: PermissionRequestInfo;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function FallbackPermissionRequest({
  request,
  onAllow,
  onReject,
  className,
}: FallbackPermissionRequestProps) {
  return (
    <div
      className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)}
      data-testid="fallback-permission-request"
    >
      <PermissionRequestTitle title={request.title || '权限请求'} subtitle={request.description} />
      <div className="my-3 flex items-start gap-2 rounded-lg bg-amber-50 px-3 py-2">
        <ShieldAlert className="mt-0.5 h-4 w-4 shrink-0 text-amber-500" />
        <p className="text-sm text-amber-800">
          此操作需要您的确认: {request.tool_name}
        </p>
      </div>
      <div className="flex gap-2">
        <button
          className="rounded-lg bg-blue-600 px-4 py-1.5 text-sm text-white hover:bg-blue-700"
          onClick={onAllow}
        >
          允许执行
        </button>
        <button
          className="rounded-lg border border-slate-300 px-4 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
          onClick={() => onReject()}
        >
          拒绝
        </button>
      </div>
    </div>
  );
}
