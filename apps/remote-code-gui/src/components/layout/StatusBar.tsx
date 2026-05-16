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

  const agentLabel = activeAgentType ?? runtimeStatus?.permission_mode ?? 'default';

  const contextPercent = contextUsage ? Math.round(contextUsage.ratio * 100) : null;

  return (
    <div className="mt-3 flex h-9 shrink-0 items-center rounded-lg border border-white/80 bg-white/80 px-4 text-xs text-rc-text-tertiary shadow-sm backdrop-blur select-none dark:border-rc-border-primary dark:bg-rc-bg-surface/80">
      <div className="flex items-center gap-4">
        {/* Agent type */}
        <span className="flex items-center gap-2">
          <span className="inline-block h-2 w-2 rounded-full bg-rc-accent-success shadow-sm" />
          <span className="font-medium text-rc-text-secondary">{agentLabel}</span>
        </span>

        {/* Model */}
        <span className="font-mono text-rc-text-tertiary">{modelName}</span>
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      <div className="flex items-center gap-4">
        {/* Token usage */}
        {lastPromptResult && (
          <span className="font-mono text-rc-text-tertiary">
            ↑ {lastPromptResult.usage.input_tokens.toLocaleString()} &nbsp;
            ↓ {lastPromptResult.usage.output_tokens.toLocaleString()}
          </span>
        )}

        {/* Context usage */}
        {contextPercent !== null && (
          <span className={`font-mono ${contextPercent > 80 ? 'text-rc-accent-warning' : 'text-rc-text-tertiary'}`}>
            Context {contextPercent}%
          </span>
        )}

        {/* Connection status */}
        <span className="flex items-center gap-2">
          <span
            className={`inline-block h-2 w-2 rounded-full ${
              runtimeStatus ? 'bg-rc-accent-success' : 'bg-rc-text-tertiary'
            }`}
          />
          <span className="text-rc-text-tertiary">
            {runtimeStatus ? 'Connected' : 'Offline'}
          </span>
        </span>
      </div>
    </div>
  );
}
