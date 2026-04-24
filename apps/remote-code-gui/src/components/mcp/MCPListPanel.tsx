import { Search } from 'lucide-react';
import { useState } from 'react';
import type { McpServerInfo } from '../../lib/types';
import { cn } from '../../lib/utils';

export interface MCPListPanelProps {
  servers: McpServerInfo[];
  onSelect?: (server: McpServerInfo) => void;
  className?: string;
}

export function MCPListPanel({ servers, onSelect, className }: MCPListPanelProps) {
  const [filter, setFilter] = useState('');

  const filtered = servers.filter((s) =>
    s.name.toLowerCase().includes(filter.toLowerCase()),
  );

  return (
    <div className={cn('flex flex-col', className)} data-testid="mcp-list-panel">
      <div className="flex items-center gap-2 border-b border-slate-200 px-3 py-2 dark:border-slate-700">
        <Search className="h-4 w-4 text-slate-400" />
        <input
          type="text"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="搜索服务器..."
          className="min-w-0 flex-1 bg-transparent text-sm outline-none"
          data-testid="mcp-list-filter"
        />
      </div>
      <div className="flex-1 overflow-auto">
        {filtered.length === 0 && (
          <p className="py-4 text-center text-sm text-slate-400">无服务器</p>
        )}
        {filtered.map((server) => (
          <button
            key={server.name}
            className="w-full px-3 py-2 text-left text-sm hover:bg-slate-50 dark:hover:bg-slate-800"
            onClick={() => onSelect?.(server)}
            data-testid={`mcp-list-item-${server.name}`}
          >
            <span className="font-medium text-slate-700">{server.name}</span>
            <span className="ml-2 text-xs text-slate-400">{server.transport}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
