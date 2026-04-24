import { Bot, Edit, Trash2 } from 'lucide-react';
import type { AgentDefinition } from './validateAgent';

export interface AgentDetailProps {
  agent: AgentDefinition;
  onEdit?: () => void;
  onDelete?: () => void;
}

export function AgentDetail({ agent, onEdit, onDelete }: AgentDetailProps) {
  return (
    <div data-testid="agent-detail" className="rounded-lg border border-slate-200 bg-white p-4">
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Bot className="h-5 w-5 text-blue-600" />
          <h3 className="text-sm font-semibold text-slate-800">{agent.name}</h3>
        </div>
        <div className="flex gap-1">
          {onEdit && (
            <button type="button" data-testid="agent-detail-edit" className="rounded p-1 hover:bg-slate-100" onClick={onEdit} title="编辑">
              <Edit className="h-4 w-4 text-slate-400" />
            </button>
          )}
          {onDelete && (
            <button type="button" data-testid="agent-detail-delete" className="rounded p-1 hover:bg-red-50" onClick={onDelete} title="删除">
              <Trash2 className="h-4 w-4 text-red-400" />
            </button>
          )}
        </div>
      </div>
      {agent.description && (
        <p className="mb-2 text-sm text-slate-600">{agent.description}</p>
      )}
      {agent.model && (
        <p className="mb-1 text-xs text-slate-500">模型: {agent.model}</p>
      )}
      {agent.tools && agent.tools.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-1">
          {agent.tools.map((tool) => (
            <span key={tool} className="rounded bg-slate-100 px-1.5 py-0.5 text-xs text-slate-600">
              {tool}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
