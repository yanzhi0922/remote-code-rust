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

  // 构建显示列表：优先使用后端返回数据，fallback 到默认值
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

  return (
    <div className="relative">
      <button
        title="选择 Agent 类型"
        onClick={() => setOpen((prev) => !prev)}
        className="inline-flex items-center gap-2 rounded-full border border-[#ddd6c8] bg-[#fcfaf5] px-3 py-1.5 text-sm font-medium text-slate-700 transition-colors hover:bg-[#f6f1e8]"
      >
        <Bot size={14} className="text-slate-500" />
        <span className="max-w-[160px] truncate">{activeLabel}</span>
        <ChevronDown size={14} className="text-slate-400" />
      </button>

      {open && (
        <>
          <button
            aria-label="关闭下拉菜单"
            className="fixed inset-0 z-10 cursor-default"
            onClick={() => setOpen(false)}
          />
          <div className="absolute bottom-full left-0 z-20 mb-2 min-w-[260px] overflow-hidden rounded-2xl border border-[#dedad2] bg-white shadow-[0_18px_42px_rgba(24,29,33,0.16)]">
            <div className="max-h-72 overflow-y-auto p-1.5">
              {/* 默认选项：Remote Claude（null） */}
              <button
                className={`flex w-full items-start gap-2 rounded-xl px-3 py-2 text-left transition-colors ${
                  activeAgentType === null
                    ? 'bg-[#ece7dc] text-slate-900'
                    : 'text-slate-700 hover:bg-[#f3efe7]'
                }`}
                onClick={() => {
                  onSelect(null);
                  setOpen(false);
                }}
              >
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium">Remote Claude（默认）</div>
                  <div className="mt-0.5 text-xs text-slate-500">内置 Agent，直接调用 provider API</div>
                </div>
              </button>

              {/* 其他 Agent 选项 */}
              {agentEntries
                .filter((entry) => entry.agentType !== 'remote_claude')
                .map((entry) => (
                  <button
                    key={entry.agentType}
                    className={`flex w-full items-start gap-2 rounded-xl px-3 py-2 text-left transition-colors ${
                      activeAgentType === entry.agentType
                        ? 'bg-[#ece7dc] text-slate-900'
                        : entry.installed
                          ? 'text-slate-700 hover:bg-[#f3efe7]'
                          : 'cursor-not-allowed text-slate-400'
                    }`}
                    onClick={() => {
                      if (!entry.installed) return;
                      onSelect(entry.agentType);
                      setOpen(false);
                    }}
                  >
                    <div className="min-w-0">
                      <div className="truncate text-sm font-medium">
                        {entry.displayName}
                        {!entry.installed && (
                          <span className="ml-2 text-xs text-slate-400">未安装</span>
                        )}
                      </div>
                      <div className="mt-0.5 text-xs text-slate-500">{entry.description}</div>
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
