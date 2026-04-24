import { MoreVertical, Terminal } from 'lucide-react';
import { useState } from 'react';
import { cn } from '../../lib/utils';

export interface MCPStdioServerMenuProps {
  serverName: string;
  command: string;
  args?: string[];
  cwd?: string;
  onConnect?: () => void;
  onDisconnect?: () => void;
  onRemove?: () => void;
  className?: string;
}

export function MCPStdioServerMenu({
  serverName,
  command,
  args,
  cwd,
  onConnect,
  onDisconnect,
  onRemove,
  className,
}: MCPStdioServerMenuProps) {
  const [open, setOpen] = useState(false);

  const fullCommand = args && args.length > 0 ? `${command} ${args.join(' ')}` : command;

  return (
    <div className={cn('relative', className)} data-testid="mcp-stdio-server-menu">
      <div className="flex items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 py-2 dark:border-slate-700">
        <Terminal className="h-4 w-4 text-amber-500" />
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium text-slate-700">{serverName}</p>
          <p className="truncate font-mono text-xs text-slate-400">{fullCommand}</p>
          {cwd && (
            <p className="truncate text-xs text-slate-400" title={cwd}>📂 {cwd}</p>
          )}
        </div>
        <button className="rounded p-1 hover:bg-slate-100" onClick={() => setOpen(!open)} title="菜单">
          <MoreVertical className="h-4 w-4 text-slate-400" />
        </button>
      </div>
      {open && (
        <div className="absolute right-0 top-full z-10 mt-1 w-40 rounded-lg border border-slate-200 bg-white shadow-lg dark:border-slate-700">
          {onConnect && (
            <button
              className="w-full px-3 py-2 text-left text-sm hover:bg-slate-50"
              onClick={() => { onConnect(); setOpen(false); }}
            >
              连接
            </button>
          )}
          {onDisconnect && (
            <button
              className="w-full px-3 py-2 text-left text-sm text-orange-600 hover:bg-orange-50"
              onClick={() => { onDisconnect(); setOpen(false); }}
            >
              断开
            </button>
          )}
          {onRemove && (
            <button
              className="w-full px-3 py-2 text-left text-sm text-red-600 hover:bg-red-50"
              onClick={() => { onRemove(); setOpen(false); }}
            >
              删除
            </button>
          )}
        </div>
      )}
    </div>
  );
}
