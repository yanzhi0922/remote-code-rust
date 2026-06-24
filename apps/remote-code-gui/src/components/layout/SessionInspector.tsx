import { Activity, Bot, Brain, Cpu, FileText, GitBranch, Network, Settings2, Shield, TerminalSquare } from 'lucide-react';
import type { ElementType, ReactNode } from 'react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';
import { useCodexStore } from '../../stores/useCodexStore';
import { collectCodexSurfaceStats } from '../../lib/codexTimeline';
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

function formatRooMode(mode: string | null | undefined, t: TFn) {
  if (!mode || mode === 'code') return t('chatInput.permission.roo.code');
  if (mode === 'architect') return t('chatInput.permission.roo.architect');
  if (mode === 'ask') return t('chatInput.permission.roo.ask');
  if (mode === 'debug') return t('chatInput.permission.roo.debug');
  if (mode === 'orchestrator') return t('chatInput.permission.roo.orchestrator');
  return mode;
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
  const conversation = useAppStore((state) => state.conversation);
  const contextUsageBySession = useAppStore((state) => state.contextUsageBySession);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const activeAgentType = useAgentStore((state) => state.activeAgentType);
  const agentStatuses = useAgentStore((state) => state.agentStatuses);
  const codexNotifications = useCodexStore((state) => state.codexNotifications);
  const codexGuardianEvents = useCodexStore((state) => state.codexGuardianEvents);
  const codexRecoverableErrors = useCodexStore((state) => state.codexRecoverableErrors);

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
  const activeProviderConfig = providerConfigs?.providers.find((config) => config.name === effectiveProviderName) ?? null;
  const claudeMapping = activeProviderConfig?.claude_model_mapping ?? {};
  const surfaceStats = useMemo(() => collectCodexSurfaceStats(conversation), [conversation]);
  const currentSessionNotifications = useMemo(
    () =>
      activeSessionId
        ? codexNotifications.filter((notification) => notification.session_id === activeSessionId).slice(-5)
        : codexNotifications.slice(-5),
    [activeSessionId, codexNotifications],
  );
  const latestCodexMethod = currentSessionNotifications[currentSessionNotifications.length - 1]?.method ?? '\u2014';
  const guardianIssueCount = codexGuardianEvents.filter((event) => !activeSessionId || event.session_id === activeSessionId).length;
  const recoverableErrorCount = codexRecoverableErrors.filter((event) => !activeSessionId || event.session_id === activeSessionId).length;

  return (
    <aside
      aria-label={t('inspector.envInfo')}
      className="flex h-full w-[326px] shrink-0 flex-col"
    >
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-rc-border-primary bg-rc-bg-elevated shadow-sm">
        <div className="flex h-14 shrink-0 items-center justify-between border-b border-rc-border-secondary/70 px-5">
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

        <InspectorSection icon={FileText} title={t('inspector.timelineSection')}>
          <InspectorRow label={t('inspector.commandsLabel')} value={String(surfaceStats.command)} />
          <InspectorRow label={t('inspector.fileChangesLabel')} value={String(surfaceStats.file)} />
          <InspectorRow label={t('inspector.mcpCallsLabel')} value={String(surfaceStats.mcp)} />
          <InspectorRow label={t('inspector.reasoningLabel')} value={String(surfaceStats.reasoning)} />
        </InspectorSection>

        {agentType === 'remote_codex' ? (
          <InspectorSection icon={Bot} title={t('inspector.codexSection')}>
            <InspectorRow label={t('inspector.latestEventLabel')} value={latestCodexMethod} />
            <InspectorRow
              label={t('inspector.guardianLabel')}
              value={String(guardianIssueCount)}
              tone={guardianIssueCount > 0 ? 'warning' : 'default'}
            />
            <InspectorRow
              label={t('inspector.recoverableErrorsLabel')}
              value={String(recoverableErrorCount)}
              tone={recoverableErrorCount > 0 ? 'warning' : 'default'}
            />
          </InspectorSection>
        ) : agentType === 'remote_roo' ? (
          <InspectorSection icon={GitBranch} title={t('inspector.rooSection')}>
            <InspectorRow label={t('inspector.rooModeLabel')} value={formatRooMode(settings?.roo_mode, t)} />
            <InspectorRow label={t('inspector.rooInteractionsLabel')} value={t('inspector.rooInteractionsValue')} />
            <InspectorRow label={t('inspector.rooHandoffLabel')} value={t('inspector.rooHandoffValue')} />
          </InspectorSection>
        ) : (
          <InspectorSection icon={Brain} title={t('inspector.claudeSection')}>
            <InspectorRow label={t('settings.opusTask')} value={claudeMapping.opus ?? t('settings.unset')} />
            <InspectorRow label={t('settings.sonnetTask')} value={claudeMapping.sonnet ?? t('settings.unset')} />
            <InspectorRow label={t('settings.haikuTask')} value={claudeMapping.haiku ?? t('settings.unset')} />
          </InspectorSection>
        )}
        </div>
      </div>
    </aside>
  );
}
