import { Globe } from 'lucide-react';

export interface LanguageOption {
  code: string;
  name: string;
}

export interface LanguagePickerProps {
  languages: LanguageOption[];
  value: string;
  onChange: (code: string) => void;
}

export function LanguagePicker({ languages, value, onChange }: LanguagePickerProps) {
  return (
    <div data-testid="language-picker" className="inline-flex items-center gap-2">
      <Globe className="h-4 w-4 text-slate-400" />
      <select
        data-testid="language-picker-select"
        title="选择语言"
        className="rounded border border-slate-200 bg-white px-2 py-1 text-sm text-slate-700"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      >
        {languages.map((lang) => (
          <option key={lang.code} value={lang.code}>
            {lang.name}
          </option>
        ))}
      </select>
    </div>
  );
}
