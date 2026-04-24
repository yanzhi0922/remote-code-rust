import { useState } from 'react';
import { Bot, ChevronDown } from 'lucide-react';
import type { AgentDefinition } from './validateAgent';

export interface AgentsMenuProps {
  agents: AgentDefinition[];
  selected?: string;
  onSelect?: (agentName: string) => void;
}

export function AgentsMenu({ agents, selected, onSelect }: AgentsMenuProps) {
  const [open, setOpen] = useState(false);

  return (
    <div data-testid="agents-menu" className="relative">
      <button
        type="button"
        data-testid="agents-menu-trigger"
        className="inline-flex items-center gap-1.5 rounded border border-slate-200 px-3 py-1.5 text-sm hover:bg-slate-50"
        onClick={() => setOpen(!open)}
      >
        <Bot className="h-4 w-4 text-slate-400" />
        <span>{selected ?? '选择Agent'}</span>
        <ChevronDown className="h-3.5 w-3.5 text-slate-400" />
      </button>
      {open && (
        <div data-testid="agents-menu-dropdown" className="absolute left-0 top-full z-10 mt-1 w-48 rounded-lg border border-slate-200 bg-white shadow-lg">
          {agents.map((agent) => (
            <button
              key={agent.name}
              type="button"
              data-testid={`agents-menu-item-${agent.name}`}
              className={`flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-slate-50 ${
                selected === agent.name ? 'bg-blue-50 text-blue-700' : 'text-slate-700'
              }`}
              onClick={() => {
                onSelect?.(agent.name);
                setOpen(false);
              }}
            >
              <Bot className="h-3.5 w-3.5" />
              {agent.name}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
