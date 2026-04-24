import { cn } from '../../lib/utils';

export interface KeyboardShortcutHintProps {
  keys: string[];
  description: string;
  className?: string;
}

export function KeyboardShortcutHint({ keys, description, className }: KeyboardShortcutHintProps) {
  return (
    <div data-testid="keyboard-shortcut-hint" className={cn('flex items-center gap-2', className)}>
      <span className="flex items-center gap-1">
        {keys.map((key, index) => (
          <span key={index} className="flex items-center gap-1">
            <kbd
              data-testid={`shortcut-key-${key}`}
              className="inline-flex min-w-[1.5rem] items-center justify-center rounded border border-slate-300 bg-slate-100 px-1.5 py-0.5 font-mono text-xs font-medium text-slate-700 shadow-sm"
            >
              {key}
            </kbd>
            {index < keys.length - 1 && (
              <span className="text-xs text-slate-400">+</span>
            )}
          </span>
        ))}
      </span>
      <span data-testid="shortcut-description" className="text-xs text-slate-500">
        {description}
      </span>
    </div>
  );
}
