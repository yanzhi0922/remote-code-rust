import { cn } from '../../lib/utils';

export interface TextInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  autoFocus?: boolean;
  multiline?: boolean;
  className?: string;
  'data-testid'?: string;
}

export function TextInput({
  value,
  onChange,
  placeholder,
  disabled,
  autoFocus,
  multiline,
  className,
  'data-testid': testId = 'text-input',
}: TextInputProps) {
  const baseClasses = 'w-full rounded border border-slate-200 bg-white px-3 py-2 text-sm text-slate-800 outline-none focus:border-blue-400 focus:ring-1 focus:ring-blue-400 disabled:opacity-50';

  if (multiline) {
    return (
      <textarea
        data-testid={testId}
        className={cn(baseClasses, 'resize-y', className)}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        autoFocus={autoFocus}
        rows={4}
      />
    );
  }

  return (
    <input
      data-testid={testId}
      type="text"
      className={cn(baseClasses, className)}
      placeholder={placeholder}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      disabled={disabled}
      autoFocus={autoFocus}
    />
  );
}
