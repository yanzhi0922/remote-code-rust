import { Terminal } from 'lucide-react';
import type { HelpCommand } from './HelpV2';

export interface CommandsProps {
  commands: HelpCommand[];
}

export function Commands({ commands }: CommandsProps) {
  if (commands.length === 0) {
    return (
      <div data-testid="commands-empty" className="py-4 text-center text-sm text-slate-400">
        没有可用命令
      </div>
    );
  }

  return (
    <div data-testid="commands-list" className="space-y-1">
      {commands.map((cmd) => (
        <div
          key={cmd.name}
          data-testid={`command-item-${cmd.name.slice(1)}`}
          className="flex items-center gap-2 rounded px-2 py-1.5 hover:bg-slate-50"
        >
          <Terminal className="h-3.5 w-3.5 shrink-0 text-slate-400" />
          <code className="text-sm font-medium text-slate-700">{cmd.name}</code>
          <span className="text-xs text-slate-500">{cmd.description}</span>
        </div>
      ))}
    </div>
  );
}
