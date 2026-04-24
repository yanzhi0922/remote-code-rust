import { Keyboard } from 'lucide-react';

export interface ConfigurableShortcutHintProps {
  action: string;
  fallback: string;
  description: string;
  parens?: boolean;
  bold?: boolean;
}

export function ConfigurableShortcutHint({
  fallback,
  description,
  parens = true,
  bold = false,
}: ConfigurableShortcutHintProps) {
  const content = (
    <span
      data-testid="configurable-shortcut-hint"
      className={`inline-flex items-center gap-1 text-xs text-slate-500 ${bold ? 'font-semibold' : ''}`}
    >
      <Keyboard className="h-3 w-3" />
      {description}
      <kbd className="rounded border border-slate-200 bg-slate-50 px-1 py-0.5 font-mono text-xs">
        {fallback}
      </kbd>
    </span>
  );

  if (parens) {
    return <span data-testid="configurable-shortcut-hint-parens">({content})</span>;
  }
  return content;
}
