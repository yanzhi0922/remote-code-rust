import { Bot, Plus } from 'lucide-react';
import type { AgentDefinition } from './validateAgent';

export interface AgentsListProps {
  agents: AgentDefinition[];
  selectedId?: string;
  onSelect?: (index: number) => void;
  onAdd?: () => void;
}

export function AgentsList({ agents, selectedId, onSelect, onAdd }: AgentsListProps) {
  return (
    <div data-testid="agents-list" className="space-y-1">
      <div className="flex items-center justify-between px-2 py-1">
        <span className="text-xs font-medium text-slate-500">Agent 列表</span>
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
      {agents.length === 0 ? (
        <div data-testid="agents-list-empty" className="py-4 text-center text-sm text-slate-400">
          暂无Agent
        </div>
      ) : (
        agents.map((agent, i) => (
          <button
            key={agent.name}
            type="button"
            data-testid={`agents-list-item-${i}`}
            className={`flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-slate-50 ${
              selectedId === agent.name ? 'bg-blue-50' : ''
            }`}
            onClick={() => onSelect?.(i)}
          >
            <Bot className="h-4 w-4 shrink-0 text-slate-400" />
            <span className="truncate text-slate-700">{agent.name}</span>
          </button>
        ))
      )}
    </div>
  );
}
