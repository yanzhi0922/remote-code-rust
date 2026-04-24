import { Bot } from 'lucide-react';

export interface AgentCardProps {
  agent: {
    name: string;
    description: string;
    model?: string;
    color?: string;
    tools: string[];
    is_builtin: boolean;
    disabled: boolean;
  };
  onClick: () => void;
  selected?: boolean;
}

export function AgentCard({ agent, onClick, selected = false }: AgentCardProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      data-testid={`agent-card-${agent.name}`}
      className={`w-full rounded-2xl border bg-white p-4 text-left transition-all hover:shadow-md ${
        selected
          ? 'border-blue-500 ring-2 ring-blue-200'
          : 'border-slate-200 hover:border-slate-300'
      } ${agent.disabled ? 'opacity-50' : ''}`}
    >
      <div className="flex items-start gap-3">
        <div className="flex items-center gap-2">
          <span
            className="h-3 w-3 rounded-full"
            style={{ backgroundColor: agent.color || '#6b7280' }}
          />
          <Bot className="h-5 w-5 text-slate-500" />
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-sm font-semibold text-slate-800">
              {agent.name}
            </span>
            {agent.is_builtin && (
              <span className="shrink-0 rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-500">
                内置
              </span>
            )}
            {agent.disabled && (
              <span className="shrink-0 rounded-full bg-red-50 px-2 py-0.5 text-xs text-red-400">
                已禁用
              </span>
            )}
          </div>

          <p className="mt-1 line-clamp-2 text-xs text-slate-500">
            {agent.description}
          </p>

          <div className="mt-2 flex items-center gap-2">
            {agent.model && (
              <span className="rounded-full bg-blue-50 px-2 py-0.5 text-xs text-blue-600">
                {agent.model}
              </span>
            )}
            <span className="rounded-full bg-slate-50 px-2 py-0.5 text-xs text-slate-500">
              {agent.tools.length} 工具
            </span>
          </div>
        </div>
      </div>
    </button>
  );
}
