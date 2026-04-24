import { Wrench } from 'lucide-react';
import { cn } from '../../../lib/utils';

export interface ToolsStepProps {
  selected: string[];
  onToggle: (tool: string) => void;
  availableTools: string[];
  className?: string;
}

export function ToolsStep({ selected, onToggle, availableTools, className }: ToolsStepProps) {
  const allSelected = availableTools.length > 0 && selected.length === availableTools.length;
  const noneSelected = selected.length === 0;

  const handleSelectAll = () => {
    if (allSelected) {
      availableTools.forEach((tool) => {
        if (selected.includes(tool)) {
          onToggle(tool);
        }
      });
    } else {
      availableTools.forEach((tool) => {
        if (!selected.includes(tool)) {
          onToggle(tool);
        }
      });
    }
  };

  return (
    <div data-testid="wizard-tools-step" className={cn('space-y-3', className)}>
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-slate-700">选择工具</h3>
        <button
          type="button"
          data-testid="select-all-tools"
          onClick={handleSelectAll}
          className="rounded-md px-2 py-1 text-xs text-blue-600 hover:bg-blue-50"
        >
          {allSelected ? '全不选' : '全选'}
        </button>
      </div>

      {availableTools.length === 0 ? (
        <p data-testid="no-tools" className="text-sm text-slate-400">
          暂无可用工具
        </p>
      ) : (
        <div data-testid="tools-list" className="space-y-2">
          {availableTools.map((tool) => {
            const isSelected = selected.includes(tool);
            return (
              <label
                key={tool}
                data-testid={`tool-item-${tool}`}
                className={cn(
                  'flex cursor-pointer items-center gap-3 rounded-lg border px-4 py-3 transition-all',
                  isSelected
                    ? 'border-blue-500 bg-blue-50'
                    : 'border-slate-200 bg-white hover:border-slate-300'
                )}
              >
                <input
                  type="checkbox"
                  checked={isSelected}
                  onChange={() => onToggle(tool)}
                  data-testid={`tool-checkbox-${tool}`}
                  className="h-4 w-4 rounded border-slate-300 text-blue-600 focus:ring-blue-500"
                />
                <Wrench className={cn('h-4 w-4', isSelected ? 'text-blue-600' : 'text-slate-400')} />
                <span className={cn('text-sm', isSelected ? 'font-medium text-blue-700' : 'text-slate-700')}>
                  {tool}
                </span>
              </label>
            );
          })}
        </div>
      )}

      <p className="text-xs text-slate-400">
        已选择 {selected.length} / {availableTools.length} 个工具
      </p>
    </div>
  );
}
