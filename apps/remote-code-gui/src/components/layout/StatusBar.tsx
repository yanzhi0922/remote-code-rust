import { useMemo } from 'react';
import { useAppStore } from '../../stores/useAppStore';

export function StatusBar() {
  const provider = useAppStore((state) => state.provider);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const sessions = useAppStore((state) => state.sessions);
  const contextUsageBySession = useAppStore((state) => state.contextUsageBySession);
  const activeAgentType = useAppStore((state) => state.activeAgentType);
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

  const agentLabel = activeAgentType ?? runtimeStatus?.permission_mode ?? 'default';

  const contextPercent = contextUsage ? Math.round(contextUsage.ratio * 100) : null;

  return (
    <div className="flex h-status-bar shrink-0 items-center border-t border-rc-border-primary bg-rc-bg-primary px-3 text-2xs text-rc-text-tertiary select-none">
      <div className="flex items-center gap-3">
        {/* Agent type */}
        <span className="flex items-center gap-1">
          <span className="inline-block h-2 w-2 rounded-full bg-rc-accent-success" />
          <span>{agentLabel}</span>
        </span>

        {/* Model */}
        <span className="text-rc-text-secondary">{modelName}</span>
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      <div className="flex items-center gap-3">
        {/* Token usage */}
        {lastPromptResult && (
          <span>
            ↑{lastPromptResult.usage.input_tokens} ↓{lastPromptResult.usage.output_tokens}
          </span>
        )}

        {/* Context usage */}
        {contextPercent !== null && (
          <span className={contextPercent > 80 ? 'text-rc-accent-warning' : ''}>
            ctx {contextPercent}%
          </span>
        )}

        {/* Connection status */}
        <span className="flex items-center gap-1">
          <span className={`inline-block h-1.5 w-1.5 rounded-full ${runtimeStatus ? 'bg-rc-accent-success' : 'bg-rc-text-tertiary'}`} />
          {runtimeStatus ? 'Connected' : 'Offline'}
        </span>
      </div>
    </div>
  );
}
