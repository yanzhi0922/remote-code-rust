import { ChevronDown, Cpu } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { AgentTypeInfo, AgentType } from '../../lib/types';

interface AgentSelectorProps {
  availableAgents: AgentTypeInfo[];
  activeAgentType: AgentType | null;
  lockedAgentType?: AgentType | null;
  lockedReason?: string;
  onSelect: (agentType: AgentType | null) => void;
  /**
   * When true, the menu is rendered open without a trigger button —
   * useful when embedding AgentSelector inline inside a parent dropdown
   * (e.g. the composer chip strip).
   */
  defaultOpen?: boolean;
}

const AGENT_TYPE_KEYS: Record<AgentType, string> = {
  remote_claude: 'claude',
  remote_roo: 'roo',
  remote_codex: 'codex',
};

export function AgentSelector({
  availableAgents,
  activeAgentType,
  lockedAgentType,
  lockedReason,
  onSelect,
  defaultOpen = false,
}: AgentSelectorProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(defaultOpen);

  const agentEntries: Array<{
    agentType: AgentType;
    displayName: string;
    description: string;
    installed: boolean;
    available: boolean;
  }> = (['remote_codex', 'remote_claude', 'remote_roo'] as const).map((type) => {
    const info = availableAgents.find((agent) => agent.agentType === type);
    const key = AGENT_TYPE_KEYS[type];
    return {
      agentType: type,
      displayName: info?.displayName ?? t(`agent.${key}.displayName`),
      description: t(`agent.${key}.description`),
      installed: info?.installed ?? type === 'remote_claude',
      available: info?.available ?? type === 'remote_claude',
    };
  });

  const activeType = activeAgentType ?? 'remote_claude';
  const activeLabel = agentEntries.find((entry) => entry.agentType === activeType)?.displayName ?? 'Claude';

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
      {!defaultOpen && (
        <button
          type="button"
          title={t('agent.selectType')}
          aria-expanded={open}
          aria-haspopup="menu"
          onClick={() => setOpen((prev) => !prev)}
          className="inline-flex h-7 items-center gap-1.5 rounded-md border border-transparent bg-transparent px-2 text-xs font-medium text-rc-text-secondary transition-colors hover:border-rc-border-primary hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          <Cpu size={14} className="text-rc-text-tertiary" />
          <span className="max-w-[150px] truncate">{activeLabel}</span>
          <ChevronDown size={14} className="text-rc-text-tertiary" />
        </button>
      )}

      {open && (
        <>
          <button
            aria-label={t('agent.closeMenu')}
            className="fixed inset-0 z-10 cursor-default"
            onClick={() => setOpen(false)}
          />
          <div role="menu" className="codex-popover absolute bottom-full left-0 z-20 mb-2 min-w-[280px]">
            <div className="max-h-72 overflow-y-auto p-1.5">
              {agentEntries.map((entry) => {
                const lockedOut = !!lockedAgentType && entry.agentType !== lockedAgentType;
                const disabled = lockedOut || !entry.installed;
                return (
                  <button
                    key={entry.agentType}
                    type="button"
                    role="menuitemradio"
                    aria-checked={activeType === entry.agentType}
                    aria-disabled={disabled}
                    title={lockedOut ? lockedReason : undefined}
                    className={`flex w-full items-start gap-2 rounded-md px-3 py-2 text-left transition-colors ${
                      activeType === entry.agentType
                        ? 'bg-rc-bg-selected text-rc-text-primary'
                        : !disabled
                          ? 'text-rc-text-primary hover:bg-rc-bg-hover'
                          : 'cursor-not-allowed text-rc-text-tertiary'
                    }`}
                    onClick={() => {
                      if (disabled) return;
                      onSelect(entry.agentType);
                      setOpen(false);
                    }}
                  >
                    {renderStatusDot(entry.installed, entry.available)}
                    <div className="min-w-0">
                      <div className="truncate text-sm font-medium">
                        {entry.displayName}
                        {!entry.installed && (
                          <span className="ml-2 text-xs text-rc-text-tertiary">{t('agent.notInstalled')}</span>
                        )}
                        {lockedOut && (
                          <span className="ml-2 text-xs text-rc-text-tertiary">{t('agent.locked')}</span>
                        )}
                      </div>
                      <div className="mt-0.5 text-xs text-rc-text-tertiary">{entry.description}</div>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
