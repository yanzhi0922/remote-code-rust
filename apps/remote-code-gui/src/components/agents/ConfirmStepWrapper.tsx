import type { ReactNode } from 'react';
import { CheckCircle, AlertCircle } from 'lucide-react';

export interface ConfirmStepWrapperProps {
  children: ReactNode;
  confirmed: boolean;
  onConfirm?: () => void;
  onCancel?: () => void;
  confirmLabel?: string;
  cancelLabel?: string;
}

export function ConfirmStepWrapper({
  children,
  confirmed,
  onConfirm,
  onCancel,
  confirmLabel = '确认',
  cancelLabel = '取消',
}: ConfirmStepWrapperProps) {
  return (
    <div data-testid="confirm-step-wrapper" className="rounded-lg border border-slate-200 p-4">
      {confirmed ? (
        <div data-testid="confirm-step-confirmed" className="flex items-center gap-2 text-green-600">
          <CheckCircle className="h-5 w-5" />
          <span className="text-sm font-medium">已确认</span>
        </div>
      ) : (
        <>
          <div className="mb-3">{children}</div>
          <div className="flex items-center gap-2">
            <AlertCircle className="h-4 w-4 text-amber-500" />
            <span className="text-sm text-slate-600">请确认以上内容</span>
            <div className="ml-auto flex gap-2">
              <button
                type="button"
                data-testid="confirm-step-cancel"
                className="rounded border border-slate-200 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
                onClick={onCancel}
              >
                {cancelLabel}
              </button>
              <button
                type="button"
                data-testid="confirm-step-confirm"
                className="rounded bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700"
                onClick={onConfirm}
              >
                {confirmLabel}
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
