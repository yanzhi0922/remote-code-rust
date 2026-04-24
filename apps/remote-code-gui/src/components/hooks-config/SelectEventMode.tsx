/**
 * SelectEventMode — 事件类型选择器组件。
 *
 * 支持 PreToolUse / PostToolUse / Notification / Stop 四种事件类型。
 */

import { cn } from '@/lib/utils';

export interface SelectEventModeProps {
  value: string;
  onChange: (value: string) => void;
  className?: string;
}

const eventOptions = [
  { value: 'PreToolUse', label: 'PreToolUse' },
  { value: 'PostToolUse', label: 'PostToolUse' },
  { value: 'Notification', label: 'Notification' },
  { value: 'Stop', label: 'Stop' },
];

export function SelectEventMode({
  value,
  onChange,
  className,
}: SelectEventModeProps) {
  return (
    <select
      data-testid="select-event-mode"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className={cn(
        'rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-sm text-slate-700',
        'focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500',
        className,
      )}
    >
      {eventOptions.map((opt) => (
        <option key={opt.value} value={opt.value}>
          {opt.label}
        </option>
      ))}
    </select>
  );
}
