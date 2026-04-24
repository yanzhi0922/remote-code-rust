import { useState } from 'react';
import { Shield, AlertTriangle, XCircle, X } from 'lucide-react';
import { cn } from '../../lib/utils';
import { checkManagedSettingsSecurity, type SecurityCheckResult } from './utils';

export interface ManagedSettingsSecurityDialogProps {
  settings: Record<string, unknown>;
  open: boolean;
  onClose: () => void;
  onApply?: (sanitized: Record<string, unknown>) => void;
}

export function ManagedSettingsSecurityDialog({
  settings,
  open,
  onClose,
  onApply,
}: ManagedSettingsSecurityDialogProps) {
  const [result] = useState<SecurityCheckResult>(() => checkManagedSettingsSecurity(settings));

  if (!open) return null;

  return (
    <div data-testid="managed-settings-security-dialog" className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="fixed inset-0 bg-black/40" onClick={onClose} data-testid="ms-security-backdrop" />
      <div className="relative z-10 w-full max-w-md rounded-lg border border-slate-200 bg-white shadow-xl">
        <div className="flex items-center justify-between border-b border-slate-100 px-4 py-3">
          <div className="flex items-center gap-2">
            <Shield className="h-5 w-5 text-blue-600" />
            <h2 className="text-sm font-semibold text-slate-800">安全检查</h2>
          </div>
          <button
            type="button"
            data-testid="ms-security-close"
            className="rounded p-1 hover:bg-slate-100"
            onClick={onClose}
            title="关闭"
          >
            <X className="h-4 w-4 text-slate-400" />
          </button>
        </div>
        <div className="p-4">
          {result.secure ? (
            <div data-testid="ms-security-pass" className="flex items-center gap-2 text-green-600">
              <Shield className="h-5 w-5" />
              <span className="text-sm font-medium">设置通过安全检查</span>
            </div>
          ) : (
            <div data-testid="ms-security-fail">
              <div className="mb-3 flex items-center gap-2 text-red-600">
                <XCircle className="h-5 w-5" />
                <span className="text-sm font-medium">发现安全问题</span>
              </div>
              {result.errors.map((error, i) => (
                <div key={i} className="mb-1 flex items-start gap-2 text-sm text-red-600">
                  <XCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                  {error}
                </div>
              ))}
            </div>
          )}

          {result.warnings.length > 0 && (
            <div className="mt-3">
              <div className="mb-1 flex items-center gap-1 text-amber-600">
                <AlertTriangle className="h-4 w-4" />
                <span className="text-xs font-medium">警告</span>
              </div>
              {result.warnings.map((warning, i) => (
                <div key={i} className="mb-1 text-xs text-amber-700">
                  {warning}
                </div>
              ))}
            </div>
          )}
        </div>
        <div className="flex justify-end gap-2 border-t border-slate-100 px-4 py-3">
          <button
            type="button"
            data-testid="ms-security-cancel"
            className="rounded border border-slate-200 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
            onClick={onClose}
          >
            取消
          </button>
          {result.secure && onApply && (
            <button
              type="button"
              data-testid="ms-security-apply"
              className={cn(
                'rounded px-3 py-1.5 text-sm font-medium text-white',
                'bg-blue-600 hover:bg-blue-700',
              )}
              onClick={() => onApply(settings)}
            >
              应用
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
