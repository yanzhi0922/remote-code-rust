import { useMemo, useState } from 'react';
import { Wrench, Search } from 'lucide-react';

export interface ToolSelectorProps {
  selectedTools: string[];
  onToggle: (tool: string) => void;
  availableTools: string[];
  searchQuery?: string;
}

export function ToolSelector({ selectedTools, onToggle, availableTools, searchQuery = '' }: ToolSelectorProps) {
  const [internalSearch, setInternalSearch] = useState(searchQuery);
  const query = searchQuery || internalSearch;

  const sortedTools = useMemo(
    () => [...availableTools].sort((a, b) => a.localeCompare(b)),
    [availableTools],
  );

  const filteredTools = useMemo(
    () => sortedTools.filter((t) => t.toLowerCase().includes(query.toLowerCase())),
    [sortedTools, query],
  );

  const allSelected = filteredTools.length > 0 && filteredTools.every((t) => selectedTools.includes(t));
  const noneSelected = filteredTools.length > 0 && filteredTools.every((t) => !selectedTools.includes(t));

  function handleToggleAll() {
    if (allSelected) {
      // Deselect all visible
      for (const t of filteredTools) {
        if (selectedTools.includes(t)) {
          onToggle(t);
        }
      }
    } else {
      // Select all visible
      for (const t of filteredTools) {
        if (!selectedTools.includes(t)) {
          onToggle(t);
        }
      }
    }
  }

  return (
    <div className="space-y-3" data-testid="tool-selector">
      <div className="flex items-center justify-between">
        <label className="flex items-center gap-1.5 text-sm font-medium text-slate-700">
          <Wrench className="h-4 w-4" />
          工具
          <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-500">
            已选 {selectedTools.length}
          </span>
        </label>
        <button
          type="button"
          onClick={handleToggleAll}
          className="text-xs text-blue-600 hover:text-blue-800"
        >
          {allSelected ? '取消全选' : '全选'}
        </button>
      </div>

      {!searchQuery && (
        <div className="relative">
          <Search className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-slate-400" />
          <input
            type="text"
            value={internalSearch}
            onChange={(e) => setInternalSearch(e.target.value)}
            placeholder="搜索工具..."
            aria-label="搜索工具"
            className="w-full rounded-lg border border-slate-300 py-2 pl-8 pr-3 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
        </div>
      )}

      <div className="max-h-48 space-y-1 overflow-y-auto rounded-lg border border-slate-200 p-2">
        {filteredTools.length === 0 ? (
          <div className="py-2 text-center text-sm text-slate-400">无匹配工具</div>
        ) : (
          filteredTools.map((tool) => (
            <label
              key={tool}
              className="flex cursor-pointer items-center gap-2 rounded-md px-2 py-1 text-sm text-slate-700 hover:bg-slate-50"
            >
              <input
                type="checkbox"
                checked={selectedTools.includes(tool)}
                onChange={() => onToggle(tool)}
                className="rounded border-slate-300 text-blue-600 focus:ring-blue-500"
              />
              <span className="font-mono text-xs">{tool}</span>
            </label>
          ))
        )}
      </div>
    </div>
  );
}
