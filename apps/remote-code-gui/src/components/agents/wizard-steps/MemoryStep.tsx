import { Brain } from 'lucide-react';
import { cn } from '../../../lib/utils';

export interface MemoryStepProps {
  enabled: boolean;
  onToggle: () => void;
  maxEntries?: number;
  onMaxEntriesChange?: (n: number) => void;
  className?: string;
}

export function MemoryStep({ enabled, onToggle, maxEntries, onMaxEntriesChange, className }: MemoryStepProps) {
  return (
    <div data-testid="wizard-memory-step" className={cn('space-y-3', className)}>
      <div className="flex items-center gap-2">
        <Brain className="h-4 w-4 text-slate-500" />
        <h3 className="text-sm font-medium text-slate-700">记忆配置</h3>
      </div>
      <p className="text-xs text-slate-500">
        启用记忆功能后，Agent 可以在会话之间保留上下文信息。
      </p>

      <div className="flex items-center justify-between rounded-lg border border-slate-200 bg-white px-4 py-3">
        <span className="text-sm text-slate-700">启用记忆</span>
        <button
          type="button"
          role="switch"
          aria-checked={enabled}
          aria-label="切换记忆功能"
          data-testid="memory-toggle"
          onClick={onToggle}
          className={cn(
            'relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors',
            enabled ? 'bg-blue-600' : 'bg-slate-200'
          )}
        >
          <span
            className={cn(
              'pointer-events-none inline-block h-5 w-5 rounded-full bg-white shadow ring-0 transition-transform',
              enabled ? 'translate-x-5' : 'translate-x-0'
            )}
          />
        </button>
      </div>

      {enabled && (
        <div data-testid="memory-settings" className="space-y-2 rounded-lg border border-slate-200 bg-white p-4">
          <label htmlFor="max-entries" className="block text-sm text-slate-700">
            最大记忆条目数
          </label>
          <input
            id="max-entries"
            type="number"
            min={1}
            max={1000}
            value={maxEntries ?? 100}
            onChange={(e) => onMaxEntriesChange?.(Number(e.target.value))}
            data-testid="max-entries-input"
            className="w-full rounded-md border border-slate-200 px-3 py-2 text-sm text-slate-800 focus:border-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-200"
          />
          <p className="text-xs text-slate-400">
            建议范围: 10 - 500 条
          </p>
        </div>
      )}
    </div>
  );
}
