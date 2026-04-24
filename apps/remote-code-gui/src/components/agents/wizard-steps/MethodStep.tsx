import { Pencil, Sparkles } from 'lucide-react';
import { cn } from '../../../lib/utils';

export interface MethodStepProps {
  value: 'manual' | 'generate';
  onChange: (method: string) => void;
  className?: string;
}

const METHODS = [
  {
    id: 'manual' as const,
    label: '手动创建',
    description: '通过表单逐步填写 Agent 配置信息',
    icon: Pencil,
  },
  {
    id: 'generate' as const,
    label: 'AI 生成',
    description: '描述需求，由 AI 自动生成 Agent 配置',
    icon: Sparkles,
  },
];

export function MethodStep({ value, onChange, className }: MethodStepProps) {
  return (
    <div data-testid="wizard-method-step" className={cn('space-y-3', className)}>
      <h3 className="text-sm font-medium text-slate-700">创建方式</h3>
      <p className="text-xs text-slate-500">
        选择如何创建你的 Agent。
      </p>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        {METHODS.map((method) => {
          const Icon = method.icon;
          const isSelected = value === method.id;
          return (
            <button
              key={method.id}
              type="button"
              data-testid={`method-option-${method.id}`}
              onClick={() => onChange(method.id)}
              className={cn(
                'flex flex-col items-center gap-3 rounded-xl border-2 p-6 transition-all hover:shadow-md',
                isSelected
                  ? 'border-blue-500 bg-blue-50 ring-2 ring-blue-200'
                  : 'border-slate-200 bg-white hover:border-slate-300'
              )}
            >
              <Icon
                className={cn(
                  'h-10 w-10',
                  isSelected ? 'text-blue-600' : 'text-slate-400'
                )}
              />
              <span
                className={cn(
                  'text-sm font-semibold',
                  isSelected ? 'text-blue-700' : 'text-slate-700'
                )}
              >
                {method.label}
              </span>
              <span className="text-center text-xs text-slate-500">
                {method.description}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
