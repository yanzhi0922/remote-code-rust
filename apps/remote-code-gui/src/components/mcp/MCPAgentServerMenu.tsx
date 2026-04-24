import { Bot, MoreVertical } from 'lucide-react';
import { useState } from 'react';
import { cn } from '../../lib/utils';

export interface MCPAgentServerMenuProps {
  serverName: string;
  agentName: string;
  onConnect?: () => void;
  onDisconnect?: () => void;
  className?: string;
}

export function MCPAgentServerMenu({ serverName, agentName, onConnect, onDisconnect, className }: MCPAgentServerMenuProps) {
  const [open, setOpen] = useState(false);

  return (
    <div className={cn('relative', className)} data-testid="mcp-agent-server-menu">
      <div className="flex items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 py-2 dark:border-slate-700">
        <Bot className="h-4 w-4 text-blue-500" />
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium text-slate-700">{serverName}</p>
          <p className="text-xs text-slate-400">{agentName}</p>
        </div>
        <button className="rounded p-1 hover:bg-slate-100" onClick={() => setOpen(!open)} title="菜单">
          <MoreVertical className="h-4 w-4 text-slate-400" />
        </button>
      </div>
      {open && (
        <div className="absolute right-0 top-full z-10 mt-1 w-40 rounded-lg border border-slate-200 bg-white shadow-lg dark:border-slate-700">
          {onConnect && (
            <button className="w-full px-3 py-2 text-left text-sm hover:bg-slate-50" onClick={() => { onConnect(); setOpen(false); }}>
              连接
            </button>
          )}
          {onDisconnect && (
            <button className="w-full px-3 py-2 text-left text-sm text-red-600 hover:bg-red-50" onClick={() => { onDisconnect(); setOpen(false); }}>
              断开
            </button>
          )}
        </div>
      )}
    </div>
  );
}
