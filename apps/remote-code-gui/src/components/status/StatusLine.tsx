/**
 * StatusLine — 底部固定状态栏组件。
 *
 * 显示提供商、模型、权限模式、Context 使用率进度条和会话 ID。
 */

import { Activity, Cpu } from 'lucide-react';

export interface StatusLineProps {
  status: {
    provider: string;
    model: string;
    permissionMode: string;
    contextUsage?: { ratio: number };
    sessionId?: string;
  };
}

export function StatusLine({ status }: StatusLineProps) {
  const { provider, model, permissionMode, contextUsage, sessionId } = status;
  const usagePct = contextUsage ? Math.round(contextUsage.ratio * 100) : undefined;

  return (
    <div
      className="fixed bottom-0 left-0 right-0 flex items-center gap-4 bg-slate-900 px-4 py-2 text-xs text-white"
      data-testid="status-line"
    >
      {/* Provider */}
      <div className="flex items-center gap-1" data-testid="status-provider">
        <Cpu className="h-3.5 w-3.5 text-slate-400" />
        <span>{provider}</span>
      </div>

      {/* Separator */}
      <span className="text-slate-600">|</span>

      {/* Model */}
      <div className="flex items-center gap-1" data-testid="status-model">
        <Activity className="h-3.5 w-3.5 text-slate-400" />
        <span>{model}</span>
      </div>

      {/* Separator */}
      <span className="text-slate-600">|</span>

      {/* Permission mode */}
      <span data-testid="status-permission">{permissionMode}</span>

      {/* Context usage */}
      {usagePct !== undefined && (
        <>
          <span className="text-slate-600">|</span>
          <div className="flex items-center gap-2" data-testid="status-context">
            <span className="text-slate-400">Ctx</span>
            <div className="h-1.5 w-20 rounded-full bg-slate-700">
              <div
                className={`h-1.5 rounded-full transition-all ${
                  usagePct > 80
                    ? 'bg-red-500'
                    : usagePct > 50
                      ? 'bg-yellow-500'
                      : 'bg-green-500'
                }`}
                style={{ width: `${Math.min(usagePct, 100)}%` }}
              />
            </div>
            <span className="text-slate-400">{usagePct}%</span>
          </div>
        </>
      )}

      {/* Session ID */}
      {sessionId && (
        <>
          <span className="text-slate-600">|</span>
          <span className="text-slate-500" data-testid="status-session">
            {sessionId.slice(0, 8)}
          </span>
        </>
      )}
    </div>
  );
}
