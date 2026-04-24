import { useState } from 'react';
import { Palette } from 'lucide-react';

const PRESET_COLORS = [
  { name: '红', value: '#ef4444' },
  { name: '橙', value: '#f97316' },
  { name: '黄', value: '#eab308' },
  { name: '绿', value: '#22c55e' },
  { name: '青', value: '#06b6d4' },
  { name: '蓝', value: '#3b82f6' },
  { name: '紫', value: '#a855f7' },
  { name: '粉', value: '#ec4899' },
  { name: '灰', value: '#6b7280' },
];

export interface ColorPickerProps {
  value: string;
  onChange: (color: string) => void;
}

export function ColorPicker({ value, onChange }: ColorPickerProps) {
  const [customColor, setCustomColor] = useState('');

  function handleCustomColorSubmit() {
    if (customColor.trim()) {
      onChange(customColor.trim());
      setCustomColor('');
    }
  }

  return (
    <div className="space-y-3" data-testid="color-picker">
      <label className="flex items-center gap-1.5 text-sm font-medium text-slate-700">
        <Palette className="h-4 w-4" />
        颜色
      </label>

      <div className="flex flex-wrap gap-2">
        {PRESET_COLORS.map((color) => (
          <button
            key={color.value}
            type="button"
            title={color.name}
            aria-label={`选择${color.name}色`}
            className={`h-7 w-7 rounded-full border-2 transition-all hover:scale-110 ${
              value === color.value
                ? 'border-slate-800 ring-2 ring-slate-300'
                : 'border-transparent'
            }`}
            style={{ backgroundColor: color.value }}
            onClick={() => onChange(color.value)}
          />
        ))}
      </div>

      <div className="flex items-center gap-2">
        <input
          type="text"
          value={customColor}
          onChange={(e) => setCustomColor(e.target.value)}
          placeholder="自定义颜色 (#hex)"
          aria-label="自定义颜色输入"
          className="w-36 rounded-lg border border-slate-300 px-3 py-1.5 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              handleCustomColorSubmit();
            }
          }}
        />
        <button
          type="button"
          onClick={handleCustomColorSubmit}
          className="rounded-lg bg-slate-100 px-3 py-1.5 text-xs font-medium text-slate-600 hover:bg-slate-200"
        >
          应用
        </button>
      </div>

      {value && (
        <div className="flex items-center gap-2 text-xs text-slate-500">
          <span>当前：</span>
          <span
            className="inline-block h-4 w-4 rounded-full border border-slate-200"
            style={{ backgroundColor: value }}
          />
          <span>{value}</span>
        </div>
      )}
    </div>
  );
}
