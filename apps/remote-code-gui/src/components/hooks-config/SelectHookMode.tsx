/**
 * SelectHookMode — Hook 模式选择器组件。
 *
 * 支持 block / pass / modify 三种模式。
 */

import { cn } from '@/lib/utils';

export interface SelectHookModeProps {
  value: string;
  onChange: (value: string) => void;
  className?: string;
}

const hookModeOptions = [
  { value: 'block', label: 'Block' },
  { value: 'pass', label: 'Pass' },
  { value: 'modify', label: 'Modify' },
];

export function SelectHookMode({
  value,
  onChange,
  className,
}: SelectHookModeProps) {
  return (
    <select
      data-testid="select-hook-mode"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      title="Hook 模式"
      className={cn(
        'rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-sm text-slate-700',
        'focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500',
        className,
      )}
    >
      {hookModeOptions.map((opt) => (
        <option key={opt.value} value={opt.value}>
          {opt.label}
        </option>
      ))}
    </select>
  );
}
