import { Moon } from 'lucide-react';

export function SessionBackgroundHint() {
  return (
    <div data-testid="session-background-hint" className="flex items-center gap-1.5 text-xs text-slate-400">
      <Moon className="h-3.5 w-3.5" />
      <span>会话在后台运行</span>
    </div>
  );
}
