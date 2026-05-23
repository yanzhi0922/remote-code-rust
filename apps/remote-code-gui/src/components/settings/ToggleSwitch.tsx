import { clsx } from 'clsx';

export interface ToggleSwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description?: string;
  disabled?: boolean;
}

export function ToggleSwitch({ checked, onChange, label, description, disabled = false }: ToggleSwitchProps) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <div className="text-sm font-medium text-rc-text-primary">{label}</div>
        {description && (
          <p className="mt-0.5 text-xs text-rc-text-tertiary">{description}</p>
        )}
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={clsx(
          'relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border border-transparent transition-colors duration-150 ease-in-out focus:outline-none focus:ring-1 focus:ring-rc-border-focus',
          checked ? 'bg-rc-accent-primary' : 'bg-rc-bg-tertiary',
          disabled && 'cursor-not-allowed opacity-50',
        )}
      >
        <span
          className={clsx(
            'pointer-events-none inline-block h-[18px] w-[18px] transform rounded-full bg-rc-bg-surface shadow-xs ring-0 transition duration-150 ease-in-out',
            checked ? 'translate-x-4' : 'translate-x-0',
          )}
        />
      </button>
    </div>
  );
}
