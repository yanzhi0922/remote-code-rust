import { Cpu, Network, Wifi, WifiOff } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { formatSensitivePath } from '../../lib/utils';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';

function ContextGauge({ ratio }: { ratio: number }) {
  const percent = Math.round(ratio * 100);
  const color =
    percent > 90
      ? 'bg-rc-accent-error'
      : percent > 75
        ? 'bg-rc-accent-warning'
        : 'bg-rc-accent-success';

  return (
    <div className="flex items-center gap-1.5">
      <div className="h-1.5 w-16 overflow-hidden rounded-full bg-rc-bg-tertiary">
        <div
          className={`h-full rounded-full transition-all duration-500 ${color}`}
          style={{ width: `${Math.min(percent, 100)}%` }}
        />
      </div>
      <span className={`font-mono ${percent > 80 ? 'text-rc-accent-warning' : ''}`}>
        ctx {percent}%
      </span>
    </div>
  );
}

function TokenUsage({
  inputTokens,
  outputTokens,
}: {
  inputTokens: number;
  outputTokens: number;
}) {
  const format = (n: number) => {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return String(n);
  };

  return (
    <span className="font-mono">
      <span className="text-rc-accent-info">↑{format(inputTokens)}</span>
      {' '}
      <span className="text-rc-accent-success">↓{format(outputTokens)}</span>
    </span>
  );
}

export function StatusBar() {
  const { t } = useTranslation();
  const provider = useAppStore((state) => state.provider);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const sessions = useAppStore((state) => state.sessions);
  const contextUsageBySession = useAppStore((state) => state.contextUsageBySession);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
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

  const projectLabel = activeProjectPath ? formatSensitivePath(activeProjectPath, privacyMode) : t('statusBar.noProject');
  const sessionLabel = activeSession ? (privacyMode ? t('statusBar.hiddenSession') : activeSession.title) : t('statusBar.noSession');

  return (
    <div className="flex h-status-bar shrink-0 items-center border-t border-rc-border-secondary bg-rc-bg-base px-3 text-[11px] text-rc-text-tertiary select-none">
      <div className="flex min-w-0 items-center gap-3">
        <span className="flex items-center gap-1.5">
          <Cpu size={12} />
          <span className="text-rc-text-secondary">{agentLabel}</span>
        </span>
        <span className="text-rc-text-secondary">{providerName}</span>
        <span className="hidden max-w-[200px] truncate font-mono text-rc-text-tertiary sm:inline">{modelName}</span>
        <span className="hidden max-w-[260px] truncate text-rc-text-secondary lg:inline">{projectLabel}</span>
        <span className="hidden max-w-[220px] truncate xl:inline">{sessionLabel}</span>
      </div>

      <div className="flex-1" />

      <div className="flex items-center gap-3">
        {lastPromptResult && (
          <TokenUsage
            inputTokens={lastPromptResult.usage.input_tokens}
            outputTokens={lastPromptResult.usage.output_tokens}
          />
        )}

        {contextUsage && <ContextGauge ratio={contextUsage.ratio} />}

        <span className={`hidden items-center gap-1.5 sm:flex ${mcpIssueCount > 0 ? 'text-rc-accent-warning' : ''}`}>
          <Network size={12} />
          <span>{mcpLabel}</span>
        </span>

        <span className="flex items-center gap-1.5">
          {runtimeStatus ? <Wifi size={12} className="text-rc-accent-success" /> : <WifiOff size={12} />}
          <span className="hidden sm:inline">{runtimeStatus ? t('statusBar.online') : t('statusBar.offline')}</span>
        </span>

        <span className="hidden items-center gap-1 text-rc-text-tertiary md:flex">
          <kbd className="rounded border border-rc-border-primary bg-rc-bg-tertiary px-1 text-[9px]">⌘K</kbd>
          <span>{t('statusBar.command')}</span>
        </span>
      </div>
    </div>
  );
}
