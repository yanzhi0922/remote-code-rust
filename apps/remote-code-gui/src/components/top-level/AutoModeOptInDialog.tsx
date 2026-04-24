import { Shield } from 'lucide-react';

export interface AutoModeOptInDialogProps {
  open: boolean;
  onAccept: () => void;
  onDecline: () => void;
  declineExits?: boolean;
}

export function AutoModeOptInDialog({ open, onAccept, onDecline, declineExits }: AutoModeOptInDialogProps) {
  if (!open) return null;

  return (
    <div data-testid="auto-mode-opt-in-dialog" className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="fixed inset-0 bg-black/40" data-testid="auto-mode-backdrop" />
      <div className="relative z-10 w-full max-w-md rounded-lg border border-slate-200 bg-white p-6 shadow-xl">
        <div className="mb-4 flex items-center gap-2">
          <Shield className="h-5 w-5 text-blue-600" />
          <h2 className="text-lg font-semibold text-slate-800">自动模式确认</h2>
        </div>
        <p className="mb-4 text-sm text-slate-600">
          自动模式让助手自动处理权限提示。助手会在执行前检查每个工具调用的风险操作和提示注入。
          标识为安全的操作将被执行，有风险的操作将被阻止。适用于长时间运行的任务。
          会话费用略高。建议仅在隔离环境中使用。
        </p>
        <div className="flex gap-2">
          <button
            type="button"
            data-testid="auto-mode-accept"
            className="rounded bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
            onClick={onAccept}
          >
            接受
          </button>
          <button
            type="button"
            data-testid="auto-mode-accept-default"
            className="rounded border border-blue-300 px-4 py-2 text-sm text-blue-700 hover:bg-blue-50"
            onClick={onAccept}
          >
            接受并设为默认
          </button>
          <button
            type="button"
            data-testid="auto-mode-decline"
            className="rounded border border-slate-200 px-4 py-2 text-sm text-slate-600 hover:bg-slate-50"
            onClick={onDecline}
          >
            {declineExits ? '退出' : '拒绝'}
          </button>
        </div>
      </div>
    </div>
  );
}
