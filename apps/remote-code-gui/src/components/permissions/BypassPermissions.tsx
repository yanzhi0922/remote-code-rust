import { useState } from 'react';
import { ShieldAlert, ShieldCheck, AlertTriangle } from 'lucide-react';

export interface BypassPermissionsProps {
  enabled: boolean;
  onToggle: () => void;
  killswitchActive: boolean;
}

export function BypassPermissions({ enabled, onToggle, killswitchActive }: BypassPermissionsProps) {
  const [pendingConfirm, setPendingConfirm] = useState(false);

  function handleToggle() {
    if (killswitchActive) return;

    if (!enabled) {
      // Enabling bypass requires confirmation
      setPendingConfirm(true);
    } else {
      // Disabling is safe, no confirmation needed
      onToggle();
    }
  }

  function handleConfirm() {
    setPendingConfirm(false);
    onToggle();
  }

  function handleCancelConfirm() {
    setPendingConfirm(false);
  }

  return (
    <div className="rounded-xl border border-slate-200 bg-white p-4" data-testid="bypass-permissions">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          {enabled ? (
            <ShieldAlert size={20} className="text-amber-500" />
          ) : (
            <ShieldCheck size={20} className="text-emerald-500" />
          )}
          <div>
            <div className="text-sm font-semibold text-slate-800">
              {enabled ? '权限已绕过' : '权限模式已启用'}
            </div>
            <div className="text-xs text-slate-500">
              {enabled
                ? '所有工具调用将自动批准，无需确认。'
                : '工具调用需要用户确认后才会执行。'}
            </div>
          </div>
        </div>

        <button
          type="button"
          onClick={handleToggle}
          disabled={killswitchActive}
          className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 ${
            enabled ? 'bg-amber-500' : 'bg-slate-300'
          }`}
          role="switch"
          aria-checked={enabled}
          aria-label="切换权限绕过"
        >
          <span
            className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out ${
              enabled ? 'translate-x-5' : 'translate-x-0'
            }`}
          />
        </button>
      </div>

      {killswitchActive && (
        <div className="mt-3 flex items-center gap-2 rounded-lg bg-red-50 p-2 text-sm text-red-700">
          <AlertTriangle size={16} />
          <span>紧急停止已激活，权限绕过已被禁用。</span>
        </div>
      )}

      {pendingConfirm && (
        <div className="mt-3 rounded-lg border border-amber-200 bg-amber-50 p-3">
          <div className="mb-2 text-sm font-medium text-amber-800">
            确认绕过权限？
          </div>
          <div className="mb-3 text-xs text-amber-700">
            绕过权限后，所有工具调用将自动批准。这可能导致不可逆的操作。
          </div>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={handleConfirm}
              className="rounded-2xl bg-amber-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-amber-700"
            >
              确认绕过
            </button>
            <button
              type="button"
              onClick={handleCancelConfirm}
              className="rounded-2xl border border-slate-300 bg-white px-3 py-1.5 text-sm font-medium text-slate-600 hover:bg-slate-50"
            >
              取消
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
