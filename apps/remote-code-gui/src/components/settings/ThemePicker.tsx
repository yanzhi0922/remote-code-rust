import { clsx } from 'clsx';
import { Monitor, Moon, Sun } from 'lucide-react';

export interface ThemePickerProps {
  value: string;
  onChange: (theme: string) => void;
}

const THEMES = [
  {
    value: 'light',
    label: '浅色',
    icon: Sun,
    colors: ['bg-white', 'bg-slate-100', 'bg-slate-200'],
  },
  {
    value: 'dark',
    label: '深色',
    icon: Moon,
    colors: ['bg-slate-800', 'bg-slate-700', 'bg-slate-600'],
  },
  {
    value: 'system',
    label: '跟随系统',
    icon: Monitor,
    colors: ['bg-white', 'bg-slate-800', 'bg-blue-500'],
  },
];

export function ThemePicker({ value, onChange }: ThemePickerProps) {
  return (
    <div className="space-y-3" data-testid="theme-picker">
      <label className="block text-sm font-medium text-slate-700">主题</label>
      <div className="flex gap-3">
        {THEMES.map((theme) => {
          const Icon = theme.icon;
          return (
            <button
              key={theme.value}
              type="button"
              onClick={() => onChange(theme.value)}
              className={clsx(
                'flex flex-col items-center gap-2 rounded-xl border-2 p-3 transition-colors',
                value === theme.value
                  ? 'border-blue-500 bg-blue-50'
                  : 'border-slate-200 bg-white hover:border-slate-300',
              )}
              data-testid={`theme-${theme.value}`}
            >
              <Icon size={20} className={value === theme.value ? 'text-blue-600' : 'text-slate-500'} />
              <span className="text-xs font-medium text-slate-700">{theme.label}</span>
              <div className="flex gap-1">
                {theme.colors.map((color, i) => (
                  <span key={i} className={clsx('inline-block h-3 w-3 rounded-full', color)} />
                ))}
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
