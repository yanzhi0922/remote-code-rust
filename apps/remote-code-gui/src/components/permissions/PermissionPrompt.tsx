import { Shield } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface PermissionOption {
  label: string;
  value: string;
  description?: string;
}

export interface PermissionPromptProps {
  title: string;
  description?: string;
  options: PermissionOption[];
  onSelect: (value: string) => void;
  className?: string;
}

export function PermissionPrompt({ title, description, options, onSelect, className }: PermissionPromptProps) {
  return (
    <div className={cn('rounded-2xl border border-orange-200 bg-white p-4', className)} data-testid="permission-prompt">
      <div className="flex items-center gap-2">
        <Shield className="h-5 w-5 text-orange-500" />
        <h4 className="font-semibold text-slate-800">{title}</h4>
      </div>
      {description && <p className="mt-1 text-sm text-slate-500">{description}</p>}
      <ul className="mt-3 space-y-2">
        {options.map((opt) => (
          <li key={opt.value}>
            <button
              className="w-full rounded-lg border border-slate-200 px-3 py-2 text-left text-sm hover:border-blue-400 hover:bg-blue-50 dark:border-slate-700"
              onClick={() => onSelect(opt.value)}
              data-testid={`permission-option-${opt.value}`}
            >
              <span className="font-medium text-slate-700">{opt.label}</span>
              {opt.description && (
                <span className="ml-2 text-xs text-slate-400">{opt.description}</span>
              )}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
