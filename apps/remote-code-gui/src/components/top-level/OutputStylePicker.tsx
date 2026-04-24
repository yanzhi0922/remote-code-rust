import { Palette } from 'lucide-react';

export interface OutputStyle {
  id: string;
  name: string;
}

export interface OutputStylePickerProps {
  styles: OutputStyle[];
  value: string;
  onChange: (styleId: string) => void;
}

export function OutputStylePicker({ styles, value, onChange }: OutputStylePickerProps) {
  return (
    <div data-testid="output-style-picker" className="inline-flex items-center gap-2">
      <Palette className="h-4 w-4 text-slate-400" />
      <select
        data-testid="output-style-select"
        title="选择输出风格"
        className="rounded border border-slate-200 bg-white px-2 py-1 text-sm text-slate-700"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      >
        {styles.map((style) => (
          <option key={style.id} value={style.id}>{style.name}</option>
        ))}
      </select>
    </div>
  );
}
