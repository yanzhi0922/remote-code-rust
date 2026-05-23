import { Cpu, Network, Wifi, WifiOff } from 'lucide-react';
import { useMemo } from 'react';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';

export function StatusBar() {
  const provider = useAppStore((state) => state.provider);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const sessions = useAppStore((state) => state.sessions);
  const contextUsageBySession = useAppStore((state) => state.contextUsageBySession);
  const activeAgentType = useAgentStore((state) => state.activeAgentType);
  const lastPromptResult = useAppStore((state) => state.lastPromptResult);

  const activeSession = useMemo(
    () => sessions.find((s) => s.id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );

  const contextUsage = activeSessionId ? contextUsageBySession[activeSessionId] : null;

  const modelName = activeSession?.model
    ?? runtimeStatus?.provider.model
    ?? provider?.model
    ?? '—';

  const agentLabel = activeAgentType ?? 'remote_claude';
  const providerName = runtimeStatus?.provider.name ?? provider?.name ?? '—';
  const mcpSummary = runtimeStatus?.mcp ?? null;
  const mcpIssueCount = mcpSummary
    ? mcpSummary.status_counts.failed + mcpSummary.status_counts.needs_auth + mcpSummary.warning_count
    : 0;
  const mcpLabel = mcpSummary
    ? `MCP ${mcpSummary.status_counts.connected}/${mcpSummary.enabled_servers}`
    : 'MCP —';

  const contextPercent = contextUsage ? Math.round(contextUsage.ratio * 100) : null;

  return (
    <div className="flex h-status-bar shrink-0 items-center border-t border-rc-border-secondary bg-rc-bg-sidebar px-3 text-[11px] text-rc-text-tertiary select-none">
      <div className="flex items-center gap-3">
        <span className="flex items-center gap-1.5">
          <Cpu size={12} />
          <span className="text-rc-text-secondary">{agentLabel}</span>
        </span>
        <span className="text-rc-text-secondary">{providerName}</span>
        <span className="max-w-[200px] truncate font-mono text-rc-text-tertiary">{modelName}</span>
      </div>

      <div className="flex-1" />

      <div className="flex items-center gap-3">
        {lastPromptResult && (
          <span className="font-mono">
            ↑{lastPromptResult.usage.input_tokens.toLocaleString()}
            {' '}↓{lastPromptResult.usage.output_tokens.toLocaleString()}
          </span>
        )}

        {contextPercent !== null && (
          <span className={contextPercent > 80 ? 'text-rc-accent-warning' : ''}>
            ctx {contextPercent}%
          </span>
        )}

        <span className={`flex items-center gap-1.5 ${mcpIssueCount > 0 ? 'text-rc-accent-warning' : ''}`}>
          <Network size={12} />
          <span>{mcpLabel}</span>
        </span>

        <span className="flex items-center gap-1.5">
          {runtimeStatus ? <Wifi size={12} className="text-rc-accent-success" /> : <WifiOff size={12} />}
          <span>{runtimeStatus ? 'Online' : 'Offline'}</span>
        </span>
      </div>
    </div>
  );
}
