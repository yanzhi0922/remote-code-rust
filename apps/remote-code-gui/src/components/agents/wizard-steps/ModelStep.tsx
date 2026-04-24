import { CheckCircle2 } from 'lucide-react';
import { cn } from '../../../lib/utils';

export interface ModelStepProps {
  value: string;
  onChange: (model: string) => void;
  availableModels: string[];
  className?: string;
}

export function ModelStep({ value, onChange, availableModels, className }: ModelStepProps) {
  return (
    <div data-testid="wizard-model-step" className={cn('space-y-3', className)}>
      <h3 className="text-sm font-medium text-slate-700">选择模型</h3>
      <p className="text-xs text-slate-500">
        选择 Agent 使用的 AI 模型。
      </p>
      {availableModels.length === 0 ? (
        <p data-testid="no-models" className="text-sm text-slate-400">
          暂无可用模型
        </p>
      ) : (
        <div data-testid="model-list" className="space-y-2">
          {availableModels.map((model) => {
            const isSelected = value === model;
            return (
              <button
                key={model}
                type="button"
                data-testid={`model-option-${model}`}
                onClick={() => onChange(model)}
                className={cn(
                  'flex w-full items-center justify-between rounded-lg border px-4 py-3 text-left transition-all',
                  isSelected
                    ? 'border-blue-500 bg-blue-50 ring-2 ring-blue-200'
                    : 'border-slate-200 bg-white hover:border-slate-300 hover:shadow-sm'
                )}
              >
                <span
                  className={cn(
                    'text-sm font-medium',
                    isSelected ? 'text-blue-700' : 'text-slate-700'
                  )}
                >
                  {model}
                </span>
                {isSelected && (
                  <CheckCircle2 className="h-5 w-5 text-blue-600" />
                )}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
