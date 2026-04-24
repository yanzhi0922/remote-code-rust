import { Settings } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface MCPSettingsProps {
  defaultTimeout?: number;
  onTimeoutChange?: (timeout: number) => void;
  className?: string;
}

export function MCPSettings({ defaultTimeout = 30, onTimeoutChange, className }: MCPSettingsProps) {
  return (
    <div className={cn('rounded-lg border border-slate-200 bg-white p-4 dark:border-slate-700', className)} data-testid="mcp-settings">
      <div className="flex items-center gap-2">
        <Settings className="h-4 w-4 text-slate-400" />
        <h3 className="text-sm font-semibold text-slate-700">MCP 设置</h3>
      </div>
      <div className="mt-3 space-y-3">
        <div>
          <label className="text-xs text-slate-500">默认超时 (秒)</label>
          <input
            type="number"
            value={defaultTimeout}
            onChange={(e) => onTimeoutChange?.(Number(e.target.value))}
            className="mt-1 w-full rounded-md border border-slate-300 px-3 py-1.5 text-sm dark:border-slate-600"
            data-testid="mcp-timeout-input"
            title="默认超时秒数"
          />
        </div>
      </div>
    </div>
  );
}
