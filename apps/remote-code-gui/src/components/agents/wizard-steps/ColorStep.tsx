import { Palette } from 'lucide-react';
import { cn } from '../../../lib/utils';

export interface ColorStepProps {
  value: string;
  onChange: (color: string) => void;
  className?: string;
}

const PRESET_COLORS = [
  '#ef4444', '#f97316', '#f59e0b', '#eab308',
  '#84cc16', '#22c55e', '#14b8a6', '#06b6d4',
  '#3b82f6', '#6366f1', '#8b5cf6', '#a855f7',
  '#d946ef', '#ec4899', '#f43f5e', '#6b7280',
];

export function ColorStep({ value, onChange, className }: ColorStepProps) {
  return (
    <div data-testid="wizard-color-step" className={cn('space-y-3', className)}>
      <div className="flex items-center gap-2">
        <Palette className="h-4 w-4 text-slate-500" />
        <h3 className="text-sm font-medium text-slate-700">选择颜色</h3>
      </div>

      <div data-testid="color-grid" className="grid grid-cols-8 gap-2">
        {PRESET_COLORS.map((color) => {
          const isSelected = value === color;
          return (
            <button
              key={color}
              type="button"
              data-testid={`color-preset-${color.replace('#', '')}`}
              onClick={() => onChange(color)}
              aria-label={`选择颜色 ${color}`}
              className={cn(
                'h-8 w-8 rounded-full border-2 transition-all hover:scale-110',
                isSelected ? 'border-slate-800 ring-2 ring-slate-300' : 'border-transparent'
              )}
              style={{ backgroundColor: color }}
            />
          );
        })}
      </div>

      <div className="flex items-center gap-3">
        <label htmlFor="custom-color" className="text-xs text-slate-500">
          自定义颜色:
        </label>
        <input
          id="custom-color"
          type="color"
          value={value || '#3b82f6'}
          onChange={(e) => onChange(e.target.value)}
          data-testid="custom-color-input"
          className="h-8 w-8 cursor-pointer rounded border border-slate-200"
        />
        <span data-testid="color-value" className="text-xs text-slate-500">
          {value || '未选择'}
        </span>
      </div>

      {value && (
        <div className="flex items-center gap-2">
          <span className="text-xs text-slate-500">预览:</span>
          <span
            data-testid="color-preview"
            className="inline-block h-6 w-6 rounded-full border border-slate-200"
            style={{ backgroundColor: value }}
          />
        </div>
      )}
    </div>
  );
}
