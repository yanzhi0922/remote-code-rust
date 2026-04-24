import { TreePine, CheckCircle, XCircle, Settings } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface GroveConfig {
  enabled: boolean;
  endpoint: string | null;
  status: 'connected' | 'disconnected' | 'error';
  lastSync: string | null;
}

export interface GroveProps {
  config: GroveConfig;
  onToggle?: () => void;
  onConfigure?: () => void;
}

export function Grove({ config, onToggle, onConfigure }: GroveProps) {
  const statusColor = config.status === 'connected'
    ? 'text-green-500'
    : config.status === 'error'
      ? 'text-red-500'
      : 'text-slate-400';

  const StatusIcon = config.status === 'connected' ? CheckCircle : XCircle;

  return (
    <div data-testid="grove-panel" className="rounded-lg border border-slate-200 bg-white p-4">
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <TreePine className="h-5 w-5 text-green-600" />
          <h3 className="text-sm font-semibold text-slate-800">Grove 集成</h3>
        </div>
        <div className="flex items-center gap-2">
          <StatusIcon className={cn('h-4 w-4', statusColor)} />
          <span className={cn('text-xs font-medium', statusColor)}>
            {config.status === 'connected' ? '已连接' : config.status === 'error' ? '错误' : '未连接'}
          </span>
        </div>
      </div>

      {config.endpoint && (
        <p className="mb-2 truncate text-xs text-slate-500">
          端点: {config.endpoint}
        </p>
      )}

      {config.lastSync && (
        <p className="mb-3 text-xs text-slate-400">
          最后同步: {config.lastSync}
        </p>
      )}

      <div className="flex gap-2">
        <button
          type="button"
          data-testid="grove-toggle"
          className={cn(
            'rounded px-3 py-1.5 text-sm font-medium',
            config.enabled
              ? 'bg-green-100 text-green-700 hover:bg-green-200'
              : 'bg-slate-100 text-slate-600 hover:bg-slate-200',
          )}
          onClick={onToggle}
        >
          {config.enabled ? '已启用' : '已禁用'}
        </button>
        <button
          type="button"
          data-testid="grove-configure"
          className="inline-flex items-center gap-1 rounded border border-slate-200 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
          onClick={onConfigure}
        >
          <Settings className="h-3.5 w-3.5" />
          配置
        </button>
      </div>
    </div>
  );
}
