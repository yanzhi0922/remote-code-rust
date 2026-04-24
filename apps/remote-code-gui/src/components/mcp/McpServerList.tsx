import { Plus, Search } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { McpServerInfo } from '../../lib/types';
import { McpServerCard } from './McpServerCard';

interface McpServerListProps {
  servers: McpServerInfo[];
  onSelectServer: (name: string) => void;
  onAddServer: () => void;
  searchQuery?: string;
}

export function McpServerList({ servers, onSelectServer, onAddServer, searchQuery: externalQuery }: McpServerListProps) {
  const [internalQuery, setInternalQuery] = useState('');
  const query = externalQuery ?? internalQuery;

  const filtered = useMemo(() => {
    if (!query.trim()) return servers;
    const lower = query.toLowerCase();
    return servers.filter(
      (s) =>
        s.name.toLowerCase().includes(lower) ||
        s.transport.toLowerCase().includes(lower),
    );
  }, [servers, query]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
          <input
            type="text"
            value={internalQuery}
            onChange={(e) => setInternalQuery(e.target.value)}
            placeholder="搜索服务器..."
            className="w-full rounded-xl border border-slate-200 bg-white py-2 pl-9 pr-3 text-sm text-slate-800 placeholder:text-slate-400 focus:border-emerald-300 focus:outline-none"
            data-testid="mcp-server-search"
          />
        </div>
        <button
          type="button"
          onClick={onAddServer}
          className="flex shrink-0 items-center gap-1 rounded-xl bg-emerald-600 px-3 py-2 text-sm font-medium text-white hover:bg-emerald-700"
          data-testid="mcp-add-server-btn"
        >
          <Plus size={14} />
          添加服务器
        </button>
      </div>

      {filtered.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-slate-300 p-8 text-center text-sm text-slate-400">
          暂无 MCP 服务器
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {filtered.map((server) => (
            <McpServerCard
              key={server.name}
              server={server}
              onClick={() => onSelectServer(server.name)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
