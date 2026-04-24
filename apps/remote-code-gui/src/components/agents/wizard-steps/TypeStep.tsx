import { Bot, Cpu, Users } from 'lucide-react';
import { cn } from '../../../lib/utils';

export interface TypeStepProps {
  value: string;
  onChange: (type: string) => void;
  className?: string;
}

const AGENT_TYPES = [
  {
    id: 'subagent',
    label: '子代理',
    description: '执行特定任务的独立代理',
    icon: Bot,
  },
  {
    id: 'worker',
    label: '工作节点',
    description: '处理具体工作负载的执行者',
    icon: Cpu,
  },
  {
    id: 'coordinator',
    label: '协调器',
    description: '管理和调度多个代理协作',
    icon: Users,
  },
] as const;

export function TypeStep({ value, onChange, className }: TypeStepProps) {
  return (
    <div data-testid="wizard-type-step" className={cn('space-y-3', className)}>
      <h3 className="text-sm font-medium text-slate-700">选择 Agent 类型</h3>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        {AGENT_TYPES.map((type) => {
          const Icon = type.icon;
          const isSelected = value === type.id;
          return (
            <button
              key={type.id}
              type="button"
              data-testid={`type-option-${type.id}`}
              onClick={() => onChange(type.id)}
              className={cn(
                'flex flex-col items-center gap-2 rounded-xl border-2 p-4 transition-all hover:shadow-md',
                isSelected
                  ? 'border-blue-500 bg-blue-50 ring-2 ring-blue-200'
                  : 'border-slate-200 bg-white hover:border-slate-300'
              )}
            >
              <Icon
                className={cn(
                  'h-8 w-8',
                  isSelected ? 'text-blue-600' : 'text-slate-400'
                )}
              />
              <span
                className={cn(
                  'text-sm font-semibold',
                  isSelected ? 'text-blue-700' : 'text-slate-700'
                )}
              >
                {type.label}
              </span>
              <span className="text-center text-xs text-slate-500">
                {type.description}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
