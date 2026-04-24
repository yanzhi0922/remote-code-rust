import { FolderOpen } from 'lucide-react';
import { cn } from '../../../lib/utils';

export interface LocationStepProps {
  value: string;
  onChange: (path: string) => void;
  className?: string;
}

export function LocationStep({ value, onChange, className }: LocationStepProps) {
  return (
    <div data-testid="wizard-location-step" className={cn('space-y-3', className)}>
      <div className="flex items-center gap-2">
        <FolderOpen className="h-4 w-4 text-slate-500" />
        <h3 className="text-sm font-medium text-slate-700">保存位置</h3>
      </div>
      <p className="text-xs text-slate-500">
        指定 Agent 配置文件的保存路径。
      </p>
      <div className="flex gap-2">
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="例如：~/.remote-code/agents/"
          data-testid="location-input"
          className="flex-1 rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-200"
        />
        <button
          type="button"
          data-testid="location-browse"
          onClick={() => {
            const path = window.prompt('输入保存路径:', value);
            if (path !== null) {
              onChange(path);
            }
          }}
          className="flex items-center gap-1 rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-600 transition-colors hover:bg-slate-50 hover:border-slate-300"
        >
          <FolderOpen className="h-4 w-4" />
          浏览
        </button>
      </div>
      {value && (
        <p data-testid="location-preview" className="text-xs text-slate-500">
          当前路径: {value}
        </p>
      )}
    </div>
  );
}
