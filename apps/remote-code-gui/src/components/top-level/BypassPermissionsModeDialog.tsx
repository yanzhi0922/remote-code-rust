import { AlertTriangle } from 'lucide-react';

export interface BypassPermissionsModeDialogProps {
  open: boolean;
  onAccept: () => void;
  onDecline: () => void;
}

export function BypassPermissionsModeDialog({ open, onAccept, onDecline }: BypassPermissionsModeDialogProps) {
  if (!open) return null;

  return (
    <div data-testid="bypass-permissions-dialog" className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="fixed inset-0 bg-black/40" data-testid="bypass-backdrop" />
      <div className="relative z-10 w-full max-w-md rounded-lg border border-red-200 bg-white p-6 shadow-xl">
        <div className="mb-4 flex items-center gap-2 text-red-600">
          <AlertTriangle className="h-5 w-5" />
          <h2 className="text-lg font-semibold">警告: 绕过权限模式</h2>
        </div>
        <p className="mb-2 text-sm text-slate-700">
          在绕过权限模式下，助手将不会在运行潜在危险命令前请求您的批准。
        </p>
        <p className="mb-4 text-sm text-slate-700">
          此模式应仅在具有受限网络访问且可轻松恢复的沙盒容器/虚拟机中使用。
          继续即表示您接受在绕过权限模式下运行时所采取操作的所有责任。
        </p>
        <div className="flex gap-2">
          <button
            type="button"
            data-testid="bypass-decline"
            className="rounded border border-slate-200 px-4 py-2 text-sm text-slate-600 hover:bg-slate-50"
            onClick={onDecline}
          >
            不，退出
          </button>
          <button
            type="button"
            data-testid="bypass-accept"
            className="rounded bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-700"
            onClick={onAccept}
          >
            是的，我接受
          </button>
        </div>
      </div>
    </div>
  );
}
