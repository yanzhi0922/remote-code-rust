import { Activity, Cpu, Network, Settings2, Shield, TerminalSquare } from 'lucide-react';
import type { ElementType, ReactNode } from 'react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';
import { formatSensitivePath } from '../../lib/utils';
import type { AgentType, FullSettings } from '../../lib/types';

type TFn = (key: string) => string;

function InspectorRow({
  label,
  value,
  tone = 'default',
}: {
  label: string;
  value: string;
  tone?: 'default' | 'success' | 'warning';
}) {
  const toneClass =
    tone === 'success'
      ? 'text-rc-accent-success'
      : tone === 'warning'
        ? 'text-rc-accent-warning'
        : 'text-rc-text-primary';

  return (
    <div className="flex items-start justify-between gap-3 border-b border-rc-border-secondary py-2 last:border-b-0">
      <span className="shrink-0 text-xs text-rc-text-tertiary">{label}</span>
      <span className={`min-w-0 truncate text-right text-xs font-medium ${toneClass}`}>{value}</span>
    </div>
  );
}

function InspectorSection({
  icon: Icon,
  title,
  children,
}: {
  icon: ElementType;
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="border-b border-rc-border-secondary px-4 py-3 last:border-b-0">
      <div className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase text-rc-text-tertiary">
        <Icon size={13} />
        {title}
      </div>
      {children}
    </section>
  );
}

function formatAgentName(agentType: string) {
  if (agentType === 'remote_codex') return 'Codex';
  if (agentType === 'remote_roo') return 'Roo';
  return 'Claude';
}

function formatAgentStatus(status: string, t: TFn) {
  if (status === 'ready') return t('inspector.ready');
  if (status === 'running') return t('inspector.running');
  if (status === 'offline') return t('inspector.offline');
  return status;
}

function formatPermissionMode(agentType: AgentType, settings: FullSettings | null, t: TFn) {
  if (agentType === 'remote_codex') {
    const approval = settings?.codex_approval_policy ?? 'on-request';
    const sandbox = settings?.codex_sandbox_mode ?? 'workspace-write';
    if (approval === 'never' && sandbox === 'danger-full-access') return t('inspector.codexPermissions.fullAutoAccess');
    if (approval === 'on-request' && sandbox === 'danger-full-access') return t('inspector.codexPermissions.fullAccess');
    if (approval === 'on-request' && sandbox === 'read-only') return t('inspector.codexPermissions.readonlySandbox');
    if (approval === 'never') return t('inspector.codexPermissions.sandboxAuto');
    return t('inspector.codexPermissions.requestApproval');
  }

  const mode = settings?.permission_mode;
  if (agentType === 'remote_roo') {
    if (mode === 'acceptEdits') return t('inspector.rooPermissions.autoApproveEdit');
    if (mode === 'bypassPermissions') return t('inspector.rooPermissions.autoApproveAll');
    if (mode === 'dontAsk') return t('inspector.rooPermissions.autoApproveRead');
    if (mode === 'plan') return t('inspector.rooPermissions.planOnly');
    return t('inspector.rooPermissions.askEveryTime');
  }

  if (mode === 'acceptEdits') return t('inspector.claudePermissions.autoEdit');
  if (mode === 'bypassPermissions') return t('inspector.claudePermissions.fullAuto');
  if (mode === 'dontAsk') return t('inspector.claudePermissions.dontAsk');
  if (mode === 'plan') return t('inspector.claudePermissions.planOnly');
  if (mode === 'default') return t('inspector.claudePermissions.defaultPerm');
  return mode ?? '\u2014';
}

export function SessionInspector() {
  const { t } = useTranslation();
  const provider = useAppStore((state) => state.provider);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const sessions = useAppStore((state) => state.sessions);
  const settings = useAppStore((state) => state.settings);
  const providerConfigs = useAppStore((state) => state.providerConfigs);
  const pendingPermission = useAppStore((state) => state.pendingPermission);
  const liveToolProgress = useAppStore((state) => state.liveToolProgress);
  const liveToolResults = useAppStore((state) => state.liveToolResults);
  const contextUsageBySession = useAppStore((state) => state.contextUsageBySession);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const activeAgentType = useAgentStore((state) => state.activeAgentType);
  const agentStatuses = useAgentStore((state) => state.agentStatuses);

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );

  const mcpSummary = runtimeStatus?.mcp ?? null;
  const contextUsage = activeSessionId ? contextUsageBySession[activeSessionId] : null;
  const agentType = activeSession?.agent_type ?? activeAgentType ?? 'remote_claude';
  const agentStatus = agentStatuses[agentType] ?? (runtimeStatus ? 'ready' : 'offline');
  const effectiveProviderName =
    providerConfigs?.active_provider ?? settings?.provider_name ?? provider?.name ?? activeSession?.provider_name ?? 'Provider';
  const effectiveModel =
    settings?.provider_model ?? provider?.model ?? activeSession?.model ?? runtimeStatus?.provider.model ?? '\u2014';

  return (
    <aside
      aria-label={t('inspector.envInfo')}
      className="hidden w-[304px] shrink-0 bg-rc-bg-chat px-3 pb-3 pt-[68px] xl:flex xl:flex-col"
    >
      <div className="flex max-h-[calc(100dvh-116px)] min-h-0 flex-col overflow-hidden rounded-xl border border-rc-border-primary bg-rc-bg-surface shadow-md">
        <div className="flex h-12 shrink-0 items-center justify-between border-b border-rc-border-secondary px-4">
          <div className="text-sm font-semibold text-rc-text-primary">{t('inspector.envInfo')}</div>
          <Settings2 size={16} className="text-rc-text-tertiary" />
        </div>

        <div className="min-h-0 flex-1 overflow-auto">
        <InspectorSection icon={TerminalSquare} title={t('inspector.sessionSection')}>
          <InspectorRow label={t('inspector.titleLabel')} value={activeSession ? (privacyMode ? t('inspector.sessionHidden') : activeSession.title) : '\u2014'} />
          <InspectorRow label={t('inspector.projectLabel')} value={activeProjectPath ? formatSensitivePath(activeProjectPath, privacyMode) : '\u2014'} />
        </InspectorSection>

        <InspectorSection icon={Cpu} title="Agent">
          <InspectorRow label={t('inspector.typeLabel')} value={formatAgentName(agentType)} />
          <InspectorRow label={t('inspector.modelLabel')} value={effectiveModel} />
          <InspectorRow
            label={effectiveProviderName}
            value={formatAgentStatus(agentStatus, t)}
            tone={agentStatus === 'ready' ? 'success' : runtimeStatus ? 'default' : 'warning'}
          />
        </InspectorSection>

        <InspectorSection icon={Shield} title={t('inspector.permissionsSection')}>
          <InspectorRow label={t('inspector.modeLabel')} value={formatPermissionMode(agentType, settings, t)} />
          <InspectorRow
            label={t('inspector.pendingLabel')}
            value={pendingPermission ? pendingPermission.tool_name : t('inspector.none')}
            tone={pendingPermission ? 'warning' : 'default'}
          />
          <InspectorRow
            label={t('inspector.contextLabel')}
            value={contextUsage ? `${Math.round(contextUsage.ratio * 100)}%` : '\u2014'}
            tone={contextUsage && contextUsage.ratio > 0.8 ? 'warning' : 'default'}
          />
        </InspectorSection>

        <InspectorSection icon={Network} title="MCP">
          <InspectorRow
            label={t('statusBar.online')}
            value={mcpSummary ? `${mcpSummary.status_counts.connected}/${mcpSummary.enabled_servers}` : '\u2014'}
            tone={mcpSummary && mcpSummary.status_counts.connected > 0 ? 'success' : 'default'}
          />
          <InspectorRow
            label={t('inspector.warningAuthLabel')}
            value={mcpSummary ? `${mcpSummary.warning_count}/${mcpSummary.status_counts.needs_auth}` : '\u2014'}
            tone={mcpSummary && (mcpSummary.warning_count > 0 || mcpSummary.status_counts.needs_auth > 0) ? 'warning' : 'default'}
          />
        </InspectorSection>

        <InspectorSection icon={Activity} title={t('inspector.eventsSection')}>
          <InspectorRow label={t('inspector.activeToolsLabel')} value={String(liveToolProgress.length)} />
          <InspectorRow label={t('inspector.toolResultsLabel')} value={String(liveToolResults.length)} />
          <InspectorRow label={t('inspector.runtimeLabel')} value={runtimeStatus ? t('statusBar.online') : t('statusBar.offline')} tone={runtimeStatus ? 'success' : 'warning'} />
        </InspectorSection>
        </div>
      </div>
    </aside>
  );
}
