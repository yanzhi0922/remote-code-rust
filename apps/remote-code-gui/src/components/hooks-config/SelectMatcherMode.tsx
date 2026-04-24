/**
 * SelectMatcherMode — 匹配器模式选择器组件。
 *
 * 支持 regex / glob / exact 三种匹配模式。
 */

import { cn } from '@/lib/utils';

export interface SelectMatcherModeProps {
  value: string;
  onChange: (value: string) => void;
  className?: string;
}

const matcherModeOptions = [
  { value: 'regex', label: 'Regex' },
  { value: 'glob', label: 'Glob' },
  { value: 'exact', label: 'Exact' },
];

export function SelectMatcherMode({
  value,
  onChange,
  className,
}: SelectMatcherModeProps) {
  return (
    <select
      data-testid="select-matcher-mode"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      title="匹配器模式"
      className={cn(
        'rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-sm text-slate-700',
        'focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500',
        className,
      )}
    >
      {matcherModeOptions.map((opt) => (
        <option key={opt.value} value={opt.value}>
          {opt.label}
        </option>
      ))}
    </select>
  );
}
