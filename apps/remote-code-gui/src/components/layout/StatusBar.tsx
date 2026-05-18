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
    ?? '未配置';

  const agentLabel = activeAgentType ?? 'remote_claude';
  const providerName = runtimeStatus?.provider.name ?? provider?.name ?? '未配置';
  const mcpSummary = runtimeStatus?.mcp ?? null;
  const mcpIssueCount = mcpSummary
    ? mcpSummary.status_counts.failed + mcpSummary.status_counts.needs_auth + mcpSummary.warning_count
    : 0;
  const mcpLabel = mcpSummary
    ? `MCP ${mcpSummary.status_counts.connected}/${mcpSummary.enabled_servers}`
    : 'MCP -';

  const contextPercent = contextUsage ? Math.round(contextUsage.ratio * 100) : null;

  return (
    <div className="mt-3 flex h-9 shrink-0 items-center rounded-lg border border-white/80 bg-white/80 px-4 text-xs text-rc-text-tertiary shadow-sm backdrop-blur select-none dark:border-rc-border-primary dark:bg-rc-bg-surface/80">
      <div className="flex items-center gap-4">
        <span className="flex items-center gap-2">
          <Cpu size={13} />
          <span className="font-medium text-rc-text-secondary">{agentLabel}</span>
        </span>

        <span className="truncate font-medium text-rc-text-secondary">{providerName}</span>
        <span className="max-w-[280px] truncate font-mono text-rc-text-tertiary">{modelName}</span>
      </div>

      <div className="flex-1" />

      <div className="flex items-center gap-4">
        {lastPromptResult && (
          <span className="font-mono text-rc-text-tertiary">
            ↑ {lastPromptResult.usage.input_tokens.toLocaleString()} &nbsp;
            ↓ {lastPromptResult.usage.output_tokens.toLocaleString()}
          </span>
        )}

        {contextPercent !== null && (
          <span className={`font-mono ${contextPercent > 80 ? 'text-rc-accent-warning' : 'text-rc-text-tertiary'}`}>
            Context {contextPercent}%
          </span>
        )}

        <span className={`flex items-center gap-2 ${mcpIssueCount > 0 ? 'text-rc-accent-warning' : 'text-rc-text-tertiary'}`}>
          <Network size={13} />
          <span>{mcpLabel}</span>
        </span>

        <span className="flex items-center gap-2">
          {runtimeStatus ? <Wifi size={13} className="text-rc-accent-success" /> : <WifiOff size={13} />}
          <span className="text-rc-text-tertiary">
            {runtimeStatus ? 'Online' : 'Offline'}
          </span>
        </span>
      </div>
    </div>
  );
}
