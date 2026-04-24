import { Sun, Moon, Monitor } from 'lucide-react';

export type Theme = 'light' | 'dark' | 'system';

export interface ThemePickerProps {
  value: Theme;
  onChange: (theme: Theme) => void;
}

const THEMES: Array<{ id: Theme; name: string; icon: typeof Sun }> = [
  { id: 'light', name: '浅色', icon: Sun },
  { id: 'dark', name: '深色', icon: Moon },
  { id: 'system', name: '跟随系统', icon: Monitor },
];

export function ThemePicker({ value, onChange }: ThemePickerProps) {
  return (
    <div data-testid="theme-picker" className="flex gap-1">
      {THEMES.map(({ id, name, icon: Icon }) => (
        <button
          key={id}
          type="button"
          data-testid={`theme-picker-${id}`}
          className={`inline-flex items-center gap-1.5 rounded px-3 py-1.5 text-sm ${
            value === id ? 'bg-blue-100 text-blue-700' : 'text-slate-500 hover:bg-slate-100'
          }`}
          onClick={() => onChange(id)}
          title={name}
        >
          <Icon className="h-4 w-4" />
          <span>{name}</span>
        </button>
      ))}
    </div>
  );
}
