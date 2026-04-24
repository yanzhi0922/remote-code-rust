import { useState, useMemo, useCallback } from 'react';
import {
  Bot,
  Plus,
  Search,
  X,
  Cpu,
  Brain,
  AlertTriangle,
  ChevronRight,
} from 'lucide-react';
import type { AgentDefinition } from './validateAgent';
import { cn } from '../../lib/utils';

export type AgentSource = 'built-in' | 'plugin' | 'project' | 'user';

export interface ResolvedAgent extends AgentDefinition {
  source: AgentSource;
  overriddenBy?: string | null;
  memoryCount?: number;
}

export interface AgentsListProps {
  agents: ResolvedAgent[];
  selectedId?: string;
  onSelect?: (index: number) => void;
  onAdd?: () => void;
  /** Filter by source */
  sourceFilter?: AgentSource | 'all';
}

const SOURCE_GROUPS: { source: AgentSource | 'all'; label: string }[] = [
  { source: 'all', label: '全部' },
  { source: 'built-in', label: '内置' },
  { source: 'plugin', label: '插件' },
  { source: 'project', label: '项目' },
  { source: 'user', label: '用户' },
];

const SOURCE_GROUP_ORDER: AgentSource[] = ['project', 'user', 'plugin', 'built-in'];

const SOURCE_LABELS: Record<AgentSource, string> = {
  'built-in': '内置',
  plugin: '插件',
  project: '项目',
  user: '用户',
};

const SOURCE_COLORS: Record<AgentSource, string> = {
  'built-in': 'bg-slate-100 text-slate-500',
  plugin: 'bg-purple-50 text-purple-600',
  project: 'bg-blue-50 text-blue-600',
  user: 'bg-emerald-50 text-emerald-600',
};

function compareAgentsByName(a: ResolvedAgent, b: ResolvedAgent): number {
  return a.name.localeCompare(b.name);
}

export function AgentsList({
  agents,
  selectedId,
  onSelect,
  onAdd,
  sourceFilter = 'all',
}: AgentsListProps) {
  const [searchQuery, setSearchQuery] = useState('');
  const [internalSourceFilter, setInternalSourceFilter] = useState<AgentSource | 'all'>(sourceFilter);

  // Sort agents by name
  const sortedAgents = useMemo(() => [...agents].sort(compareAgentsByName), [agents]);

  // Apply source filter
  const sourceFilteredAgents = useMemo(() => {
    if (internalSourceFilter === 'all') return sortedAgents;
    return sortedAgents.filter((a) => a.source === internalSourceFilter);
  }, [sortedAgents, internalSourceFilter]);

  // Apply search filter
  const filteredAgents = useMemo(() => {
    if (!searchQuery.trim()) return sourceFilteredAgents;
    const q = searchQuery.toLowerCase();
    return sourceFilteredAgents.filter(
      (a) =>
        a.name.toLowerCase().includes(q) ||
        (a.description && a.description.toLowerCase().includes(q)) ||
        (a.model && a.model.toLowerCase().includes(q)),
    );
  }, [sourceFilteredAgents, searchQuery]);

  // Group agents by source
  const groupedAgents = useMemo(() => {
    const groups = new Map<AgentSource, ResolvedAgent[]>();
    for (const agent of filteredAgents) {
      const existing = groups.get(agent.source) || [];
      existing.push(agent);
      groups.set(agent.source, existing);
    }
    return groups;
  }, [filteredAgents]);

  // Active filter counts
  const sourceCounts = useMemo(() => {
    const counts: Record<AgentSource | 'all', number> = { all: sortedAgents.length, 'built-in': 0, plugin: 0, project: 0, user: 0 };
    for (const a of sortedAgents) {
      counts[a.source] = (counts[a.source] || 0) + 1;
    }
    return counts;
  }, [sortedAgents]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Escape' && searchQuery) {
        e.preventDefault();
        setSearchQuery('');
      }
    },
    [searchQuery],
  );

  return (
    <div data-testid="agents-list" className="flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2">
        <span className="text-xs font-medium text-slate-500">
          Agent 列表 ({filteredAgents.length})
        </span>
        {onAdd && (
          <button
            type="button"
            data-testid="agents-list-add"
            className="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-xs text-blue-600 hover:bg-blue-50"
            onClick={onAdd}
          >
            <Plus className="h-3 w-3" />
            新建
          </button>
        )}
      </div>

      {/* Source filter tabs */}
      <div className="flex gap-1 px-3 pb-2 overflow-x-auto" data-testid="agents-source-filter">
        {SOURCE_GROUPS.map((group) => (
          <button
            key={group.source}
            type="button"
            className={cn(
              'flex items-center gap-1 rounded-full px-2.5 py-1 text-[11px] font-medium transition-colors whitespace-nowrap',
              internalSourceFilter === group.source
                ? 'bg-blue-100 text-blue-700'
                : 'bg-slate-50 text-slate-500 hover:bg-slate-100',
            )}
            onClick={() => setInternalSourceFilter(group.source)}
            data-testid={`agents-filter-${group.source}`}
          >
            {group.label}
            {sourceCounts[group.source] > 0 && (
              <span className="text-[10px] opacity-60">{sourceCounts[group.source]}</span>
            )}
          </button>
        ))}
      </div>

      {/* Search input */}
      <div className="relative px-3 pb-2">
        <Search className="absolute left-5 top-1/2 -translate-y-1/2 h-3 w-3 text-slate-400" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="搜索 Agent..."
          className="w-full rounded-lg border border-slate-200 py-1.5 pl-8 pr-7 text-xs focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          data-testid="agents-search-input"
        />
        {searchQuery && (
          <button
          type="button"
          onClick={() => setSearchQuery('')}
          className="absolute right-5 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600"
          aria-label="清除搜索"
        >
            <X className="h-3 w-3" />
          </button>
        )}
      </div>

      {/* Agent list */}
      <div className="flex-1 overflow-y-auto">
        {filteredAgents.length === 0 ? (
          <div data-testid="agents-list-empty" className="py-8 text-center">
            <Bot className="mx-auto mb-2 h-8 w-8 text-slate-300" />
            <p className="text-sm text-slate-400">暂无 Agent</p>
            <p className="mt-1 text-xs text-slate-300">
              创建专门的子代理，让 AI 可以委派任务。
            </p>
            {onAdd && (
              <button
                type="button"
                onClick={onAdd}
                className="mt-3 inline-flex items-center gap-1 rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-700"
                data-testid="agents-list-create-new"
              >
                <Plus className="h-3 w-3" />
                创建新 Agent
              </button>
            )}
          </div>
        ) : (
          // Grouped by source
          SOURCE_GROUP_ORDER.map((source) => {
            const groupAgents = groupedAgents.get(source);
            if (!groupAgents || groupAgents.length === 0) return null;

            return (
              <div key={source} className="mb-2" data-testid={`agents-group-${source}`}>
                {/* Group header */}
                <div className="flex items-center gap-1.5 px-3 py-1">
                  <span
                    className={cn(
                      'inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium',
                      SOURCE_COLORS[source],
                    )}
                  >
                    {SOURCE_LABELS[source]}
                  </span>
                  <span className="text-[10px] text-slate-300">
                    {groupAgents.length} 个
                  </span>
                </div>
                {/* Agent items */}
                {groupAgents.map((agent) => {
                  const isSelected = selectedId === agent.name;
                  const isOverridden = !!agent.overriddenBy;
                  const isBuiltIn = agent.source === 'built-in';

                  return (
                    <button
                      key={`${agent.name}-${agent.source}`}
                      type="button"
                      data-testid={`agents-list-item-${agent.name}`}
                      className={cn(
                        'flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors',
                        isSelected ? 'bg-blue-50' : 'hover:bg-slate-50',
                        isOverridden && !isSelected && 'opacity-50',
                      )}
                      onClick={() => onSelect?.(agents.indexOf(agent))}
                    >
                      <Bot
                        className={cn(
                          'h-4 w-4 shrink-0',
                          isBuiltIn ? 'text-slate-300' : 'text-slate-400',
                        )}
                      />
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-1.5">
                          <span className="truncate text-slate-700 text-xs font-medium">
                            {agent.name}
                          </span>
                          {/* Model badge */}
                          {agent.model && (
                            <span className="inline-flex items-center gap-0.5 text-[10px] text-slate-400">
                              <Cpu className="h-2.5 w-2.5" />
                              {agent.model}
                            </span>
                          )}
                          {/* Memory count */}
                          {agent.memoryCount !== undefined && agent.memoryCount > 0 && (
                            <span className="inline-flex items-center gap-0.5 text-[10px] text-slate-400">
                              <Brain className="h-2.5 w-2.5" />
                              {agent.memoryCount}
                            </span>
                          )}
                          {/* Override warning */}
                          {isOverridden && (
                            <span
                              className="inline-flex items-center gap-0.5 text-[10px] text-amber-500"
                              data-testid={`agent-overridden-${agent.name}`}
                            >
                              <AlertTriangle className="h-2.5 w-2.5" />
                              被 {agent.overriddenBy} 覆盖
                            </span>
                          )}
                        </div>
                        {/* Description */}
                        {agent.description && (
                          <p className="mt-0.5 truncate text-[11px] text-slate-400">
                            {agent.description}
                          </p>
                        )}
                      </div>
                      {isSelected && (
                        <ChevronRight className="h-3 w-3 shrink-0 text-blue-400" />
                      )}
                    </button>
                  );
                })}
              </div>
            );
          })
        )}
      </div>

      {/* Footer: Create new agent */}
      {onAdd && filteredAgents.length > 0 && (
        <div className="border-t border-slate-100 px-3 py-2">
          <button
            type="button"
            onClick={onAdd}
            className="flex w-full items-center justify-center gap-1 rounded-lg border border-dashed border-slate-200 py-2 text-xs text-slate-400 transition-colors hover:border-blue-300 hover:bg-blue-50 hover:text-blue-500"
            data-testid="agents-list-create-new-bottom"
          >
            <Plus className="h-3 w-3" />
            创建新 Agent
          </button>
        </div>
      )}
    </div>
  );
}
