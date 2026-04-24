import { CheckCircle2, ArrowLeft } from 'lucide-react';
import { cn } from '../../../lib/utils';

export interface ConfirmStepProps {
  config: {
    name: string;
    type: string;
    model: string;
    tools: string[];
    color: string;
  };
  onConfirm: () => void;
  onBack: () => void;
  className?: string;
}

export function ConfirmStep({ config, onConfirm, onBack, className }: ConfirmStepProps) {
  return (
    <div data-testid="wizard-confirm-step" className={cn('space-y-4', className)}>
      <h3 className="text-sm font-medium text-slate-700">确认配置</h3>
      <p className="text-xs text-slate-500">
        请确认以下 Agent 配置信息无误。
      </p>

      <div data-testid="config-summary" className="space-y-3 rounded-lg border border-slate-200 bg-white p-4">
        <div className="flex items-center gap-2">
          <span
            data-testid="summary-color"
            className="inline-block h-4 w-4 rounded-full border border-slate-200"
            style={{ backgroundColor: config.color || '#6b7280' }}
          />
          <span data-testid="summary-name" className="text-sm font-semibold text-slate-800">
            {config.name}
          </span>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <span className="text-xs text-slate-400">类型</span>
            <p data-testid="summary-type" className="text-sm text-slate-700">{config.type}</p>
          </div>
          <div>
            <span className="text-xs text-slate-400">模型</span>
            <p data-testid="summary-model" className="text-sm text-slate-700">{config.model}</p>
          </div>
        </div>

        <div>
          <span className="text-xs text-slate-400">工具</span>
          <div data-testid="summary-tools" className="mt-1 flex flex-wrap gap-1">
            {config.tools.length > 0 ? (
              config.tools.map((tool) => (
                <span
                  key={tool}
                  className="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-600"
                >
                  {tool}
                </span>
              ))
            ) : (
              <span className="text-xs text-slate-400">未选择工具</span>
            )}
          </div>
        </div>
      </div>

      <div className="flex items-center justify-between pt-2">
        <button
          type="button"
          data-testid="confirm-back"
          onClick={onBack}
          className="flex items-center gap-1 rounded-lg border border-slate-200 px-4 py-2 text-sm text-slate-600 transition-colors hover:bg-slate-50"
        >
          <ArrowLeft className="h-4 w-4" />
          返回
        </button>
        <button
          type="button"
          data-testid="confirm-button"
          onClick={onConfirm}
          className="flex items-center gap-1 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700"
        >
          <CheckCircle2 className="h-4 w-4" />
          确认创建
        </button>
      </div>
    </div>
  );
}
