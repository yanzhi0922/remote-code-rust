import { clsx } from 'clsx';

export interface OutputStylePickerProps {
  value: string;
  onChange: (style: string) => void;
}

const OUTPUT_STYLES = [
  {
    value: 'default',
    label: '默认',
    preview: '标准的输出格式，包含详细说明和代码块。',
  },
  {
    value: 'concise',
    label: '简洁',
    preview: '精简输出，仅包含关键信息。',
  },
  {
    value: 'detailed',
    label: '详细',
    preview: '包含完整的解释、示例和边界情况分析。',
  },
  {
    value: 'json',
    label: 'JSON',
    preview: '结构化 JSON 格式输出，便于程序解析。',
  },
];

export function OutputStylePicker({ value, onChange }: OutputStylePickerProps) {
  return (
    <div className="space-y-3" data-testid="output-style-picker">
      <label className="block text-sm font-medium text-slate-700">输出风格</label>
      <div className="grid grid-cols-2 gap-3">
        {OUTPUT_STYLES.map((style) => (
          <button
            key={style.value}
            type="button"
            onClick={() => onChange(style.value)}
            className={clsx(
              'flex flex-col gap-1.5 rounded-xl border-2 p-3 text-left transition-colors',
              value === style.value
                ? 'border-blue-500 bg-blue-50'
                : 'border-slate-200 bg-white hover:border-slate-300',
            )}
            data-testid={`style-${style.value}`}
          >
            <span className="text-sm font-medium text-slate-800">{style.label}</span>
            <span className="line-clamp-2 text-xs text-slate-500">{style.preview}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
