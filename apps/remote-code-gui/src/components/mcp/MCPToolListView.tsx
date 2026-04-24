import { ChevronRight, Search, Wrench } from 'lucide-react';
import { useMemo, useState } from 'react';
import { cn } from '../../lib/utils';
import type { McpToolInfo } from '../../lib/types';

export interface MCPToolListViewProps {
  tools: McpToolInfo[];
  serverName: string;
  onSelectTool: (name: string) => void;
  className?: string;
}

function truncate(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text;
  return text.slice(0, maxLen) + '...';
}

export function MCPToolListView({ tools, serverName, onSelectTool, className }: MCPToolListViewProps) {
  const [query, setQuery] = useState('');

  const filtered = useMemo(() => {
    if (!query.trim()) return tools;
    const lower = query.toLowerCase();
    return tools.filter(
      (t) =>
        t.name.toLowerCase().includes(lower) ||
        (t.description != null && t.description.toLowerCase().includes(lower)),
    );
  }, [tools, query]);

  return (
    <div className={cn('flex flex-col gap-3', className)} data-testid="mcp-tool-list-view">
      {/* Header */}
      <div className="flex items-center gap-2">
        <div className="flex items-center gap-1.5 text-sm font-medium text-slate-700">
          <Wrench size={14} />
          {serverName} 的工具
        </div>
        <span className="rounded-full bg-blue-50 px-2 py-0.5 text-xs text-blue-600">
          {tools.length}
        </span>
      </div>

      {/* Search */}
      <div className="relative">
        <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="搜索工具..."
          className="w-full rounded-xl border border-slate-200 bg-white py-2 pl-9 pr-3 text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-300 focus:outline-none"
          data-testid="mcp-tool-list-search"
          title="搜索工具"
        />
      </div>

      {/* Tool list */}
      {filtered.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-slate-300 p-6 text-center text-sm text-slate-400">
          {tools.length === 0 ? '该服务器没有可用工具' : '没有匹配的工具'}
        </div>
      ) : (
        <div className="flex flex-col gap-1.5">
          {filtered.map((tool) => (
            <button
              key={tool.name}
              type="button"
              onClick={() => onSelectTool(tool.name)}
              className="flex w-full items-center gap-3 rounded-xl border border-slate-200 bg-white p-3 text-left transition-colors hover:border-blue-200 hover:bg-blue-50"
              data-testid={`mcp-tool-list-item-${tool.name}`}
            >
              <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-blue-50">
                <Wrench size={14} className="text-blue-600" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="font-medium text-slate-800">{tool.name}</div>
                {tool.description && (
                  <div className="mt-0.5 text-xs text-slate-500">
                    {truncate(tool.description, 80)}
                  </div>
                )}
              </div>
              <ChevronRight size={14} className="shrink-0 text-slate-400" />
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
