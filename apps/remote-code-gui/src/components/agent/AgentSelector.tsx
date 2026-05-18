import { Bot, ChevronDown } from 'lucide-react';
import { useState } from 'react';
import type { AgentTypeInfo, AgentType } from '../../lib/types';

/** Agent 类型对应的默认显示信息（后端未返回时用作 fallback） */
const AGENT_DEFAULTS: Record<AgentType, { displayName: string; description: string }> = {
  remote_claude: { displayName: 'Remote Claude', description: '内置 Agent，直接调用 provider API' },
  remote_roo: { displayName: 'Remote Roo', description: 'Rust 原生 in-process Agent' },
  remote_codex: { displayName: 'Remote Codex', description: 'Rust 原生 in-process Agent' },
};

interface AgentSelectorProps {
  availableAgents: AgentTypeInfo[];
  activeAgentType: AgentType | null;
  onSelect: (agentType: AgentType | null) => void;
}

export function AgentSelector({ availableAgents, activeAgentType, onSelect }: AgentSelectorProps) {
  const [open, setOpen] = useState(false);

  const agentEntries: Array<{
    agentType: AgentType;
    displayName: string;
    description: string;
    installed: boolean;
    available: boolean;
  }> = (['remote_claude', 'remote_roo', 'remote_codex'] as const).map((type) => {
    const info = availableAgents.find((agent) => agent.agentType === type);
    const defaults = AGENT_DEFAULTS[type];
    return {
      agentType: type,
      displayName: info?.displayName ?? defaults.displayName,
      description: defaults.description,
      installed: info?.installed ?? type === 'remote_claude',
      available: info?.available ?? type === 'remote_claude',
    };
  });

  const activeLabel =
    activeAgentType === null
      ? 'Remote Claude'
      : agentEntries.find((entry) => entry.agentType === activeAgentType)?.displayName ?? 'Remote Claude';

  const renderStatusDot = (installed: boolean, available: boolean) => {
    const colorClass = !installed
      ? 'bg-rc-text-tertiary'
      : available
        ? 'bg-rc-accent-success'
        : 'bg-rc-accent-warning';

    return <span className={`mt-1 h-2 w-2 shrink-0 rounded-full ${colorClass}`} />;
  };

  return (
    <div className="relative">
      <button
        title="选择 Agent 类型"
        onClick={() => setOpen((prev) => !prev)}
        className="inline-flex items-center gap-2 rounded-lg border border-rc-border-primary bg-rc-bg-surface px-3 py-1.5 text-sm font-medium text-rc-text-secondary transition-colors hover:border-rc-border-hover hover:bg-rc-bg-hover hover:text-rc-text-primary"
      >
        <Bot size={14} className="text-rc-text-tertiary" />
        <span className="max-w-[160px] truncate">{activeLabel}</span>
        <ChevronDown size={14} className="text-rc-text-tertiary" />
      </button>

      {open && (
        <>
          <button
            aria-label="关闭下拉菜单"
            className="fixed inset-0 z-10 cursor-default"
            onClick={() => setOpen(false)}
          />
          <div className="absolute bottom-full left-0 z-20 mb-2 min-w-[280px] overflow-hidden rounded-lg border border-rc-border-primary bg-rc-bg-surface shadow-xl">
            <div className="max-h-72 overflow-y-auto p-1.5">
              <button
                className={`flex w-full items-start gap-2 rounded-md px-3 py-2 text-left transition-colors ${
                  activeAgentType === null
                    ? 'bg-rc-bg-selected text-rc-text-primary'
                    : 'text-rc-text-primary hover:bg-rc-bg-hover'
                }`}
                onClick={() => {
                  onSelect(null);
                  setOpen(false);
                }}
              >
                {renderStatusDot(true, true)}
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium">Remote Claude（默认）</div>
                  <div className="mt-0.5 text-xs text-rc-text-tertiary">内置 Agent，直接调用 provider API</div>
                </div>
              </button>

              {agentEntries
                .filter((entry) => entry.agentType !== 'remote_claude')
                .map((entry) => (
                  <button
                    key={entry.agentType}
                    className={`flex w-full items-start gap-2 rounded-md px-3 py-2 text-left transition-colors ${
                      activeAgentType === entry.agentType
                        ? 'bg-rc-bg-selected text-rc-text-primary'
                        : entry.installed
                          ? 'text-rc-text-primary hover:bg-rc-bg-hover'
                          : 'cursor-not-allowed text-rc-text-tertiary'
                    }`}
                    onClick={() => {
                      if (!entry.installed) return;
                      onSelect(entry.agentType);
                      setOpen(false);
                    }}
                  >
                    {renderStatusDot(entry.installed, entry.available)}
                    <div className="min-w-0">
                      <div className="truncate text-sm font-medium">
                        {entry.displayName}
                        {!entry.installed && (
                          <span className="ml-2 text-xs text-rc-text-tertiary">未安装</span>
                        )}
                      </div>
                      <div className="mt-0.5 text-xs text-rc-text-tertiary">{entry.description}</div>
                    </div>
                  </button>
                ))}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
