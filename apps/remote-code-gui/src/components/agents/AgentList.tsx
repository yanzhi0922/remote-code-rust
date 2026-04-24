import { useMemo, useState } from 'react';
import { Plus, Search } from 'lucide-react';
import { AgentCard } from './AgentCard';
import type { AgentCardProps } from './AgentCard';

type AgentItem = AgentCardProps['agent'];

export interface AgentListProps {
  agents: AgentItem[];
  onSelectAgent: (name: string) => void;
  onCreateAgent: () => void;
  searchQuery?: string;
}

export function AgentList({ agents, onSelectAgent, onCreateAgent, searchQuery = '' }: AgentListProps) {
  const [internalSearch, setInternalSearch] = useState(searchQuery);
  const query = searchQuery || internalSearch;

  const filteredAgents = useMemo(
    () =>
      agents.filter(
        (a) =>
          a.name.toLowerCase().includes(query.toLowerCase()) ||
          a.description.toLowerCase().includes(query.toLowerCase()),
      ),
    [agents, query],
  );

  return (
    <div className="flex flex-col gap-4" data-testid="agent-list">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-slate-800">Agent 管理</h2>
        <button
          type="button"
          onClick={onCreateAgent}
          className="flex items-center gap-1.5 rounded-xl bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
        >
          <Plus className="h-4 w-4" />
          创建 Agent
        </button>
      </div>

      {!searchQuery && (
        <div className="relative">
          <Search className="absolute left-3 top-2.5 h-4 w-4 text-slate-400" />
          <input
            type="text"
            value={internalSearch}
            onChange={(e) => setInternalSearch(e.target.value)}
            placeholder="搜索 Agent..."
            aria-label="搜索 Agent"
            className="w-full rounded-xl border border-slate-300 bg-white py-2 pl-10 pr-4 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
        </div>
      )}

      {filteredAgents.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-slate-300 py-12 text-center">
          <p className="text-sm text-slate-400">暂无自定义 Agent</p>
          <button
            type="button"
            onClick={onCreateAgent}
            className="mt-3 text-sm text-blue-600 hover:text-blue-800"
          >
            创建第一个 Agent
          </button>
        </div>
      ) : (
        <div className="grid gap-3">
          {filteredAgents.map((agent) => (
            <AgentCard
              key={agent.name}
              agent={agent}
              onClick={() => onSelectAgent(agent.name)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
