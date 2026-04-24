import { X } from 'lucide-react';
import type { PermissionRequestInfo } from '../../lib/types';
import { cn } from '../../lib/utils';

export interface PermissionDialogProps {
  request: PermissionRequestInfo | null;
  onAllow: () => void;
  onReject: (feedback?: string) => void;
  className?: string;
}

export function PermissionDialog({ request, onAllow, onReject, className }: PermissionDialogProps) {
  if (!request) return null;

  return (
    <div className={cn('fixed inset-0 z-50 flex items-center justify-center bg-black/50', className)} data-testid="permission-dialog">
      <div className="w-full max-w-lg rounded-2xl bg-white p-6 shadow-xl">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold text-slate-800">{request.title || '权限请求'}</h3>
          <button className="rounded-md p-1 hover:bg-slate-100" onClick={() => onReject()} title="关闭">
            <X className="h-4 w-4 text-slate-400" />
          </button>
        </div>
        <p className="mt-2 text-sm text-slate-600">{request.description}</p>
        <div className="mt-4 flex gap-2">
          <button className="rounded-lg bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700" onClick={onAllow}>允许执行</button>
          <button className="rounded-lg border border-slate-300 px-4 py-2 text-sm text-slate-600 hover:bg-slate-50" onClick={() => onReject()}>拒绝</button>
        </div>
      </div>
    </div>
  );
}
