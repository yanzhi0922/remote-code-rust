import { AlertTriangle } from 'lucide-react';

export interface KeybindingConflict {
  action: string;
  shortcut: string;
  conflictingAction: string;
}

export interface KeybindingWarningsProps {
  conflicts: KeybindingConflict[];
}

export function KeybindingWarnings({ conflicts }: KeybindingWarningsProps) {
  if (conflicts.length === 0) return null;

  return (
    <div data-testid="keybinding-warnings" className="space-y-2">
      <div className="flex items-center gap-1.5 text-amber-600">
        <AlertTriangle className="h-4 w-4" />
        <span className="text-sm font-medium">快捷键冲突</span>
      </div>
      {conflicts.map((conflict, i) => (
        <div
          key={i}
          data-testid={`keybinding-conflict-${i}`}
          className="rounded border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-700"
        >
          <kbd className="rounded bg-amber-100 px-1 py-0.5 font-mono text-xs">{conflict.shortcut}</kbd>
          {' '}被 <strong>{conflict.action}</strong> 和 <strong>{conflict.conflictingAction}</strong> 同时使用
        </div>
      ))}
    </div>
  );
}
