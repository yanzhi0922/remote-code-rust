import { cn } from '../../../lib/utils';

export interface DescriptionStepProps {
  value: string;
  onChange: (desc: string) => void;
  className?: string;
}

export function DescriptionStep({ value, onChange, className }: DescriptionStepProps) {
  return (
    <div data-testid="wizard-description-step" className={cn('space-y-3', className)}>
      <h3 className="text-sm font-medium text-slate-700">Agent 描述</h3>
      <p className="text-xs text-slate-500">
        描述这个 Agent 的用途和功能，帮助用户理解它的作用。
      </p>
      <textarea
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="例如：一个擅长代码审查的助手，能够分析代码质量并提供改进建议..."
        rows={4}
        data-testid="description-input"
        className="w-full resize-none rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-200"
      />
      <p className="text-right text-xs text-slate-400">
        {value.length} 字符
      </p>
    </div>
  );
}
