import { Activity, Cpu, FolderGit2, Network, Settings2, Shield, TerminalSquare } from 'lucide-react';
import type { ElementType, ReactNode } from 'react';
import { useMemo } from 'react';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';
import { formatSensitivePath } from '../../lib/utils';

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
    <div className="flex items-start justify-between gap-3 border-b border-rc-border-secondary py-2.5 last:border-b-0">
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
    <section className="border-b border-rc-border-secondary px-4 py-3.5 last:border-b-0">
      <div className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase text-rc-text-tertiary">
        <Icon size={13} />
        {title}
      </div>
      {children}
    </section>
  );
}

export function SessionInspector() {
  const provider = useAppStore((state) => state.provider);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const sessions = useAppStore((state) => state.sessions);
  const settings = useAppStore((state) => state.settings);
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
  const agentType = activeAgentType ?? 'remote_claude';
  const agentStatus = agentStatuses[agentType] ?? (runtimeStatus ? 'ready' : 'offline');

  return (
    <aside
      aria-label="Environment information"
      className="hidden w-[324px] shrink-0 border-l border-rc-border-secondary bg-rc-bg-chat px-4 py-4 xl:flex xl:flex-col"
    >
      <div className="min-h-0 overflow-hidden rounded-lg border border-rc-border-secondary bg-rc-bg-surface shadow-md">
        <div className="flex h-11 items-center justify-between border-b border-rc-border-secondary px-4">
          <div className="text-sm font-semibold text-rc-text-primary">环境信息</div>
          <Settings2 size={16} className="text-rc-text-tertiary" />
        </div>

        <div className="max-h-[calc(100dvh-112px)] overflow-auto">
          <InspectorSection icon={TerminalSquare} title="Session">
            <InspectorRow label="Title" value={activeSession ? (privacyMode ? 'Hidden session' : activeSession.title) : '—'} />
            <InspectorRow label="ID" value={activeSessionId ? activeSessionId.slice(0, 8) : '—'} />
            <InspectorRow label="Project" value={activeProjectPath ? formatSensitivePath(activeProjectPath, privacyMode) : '—'} />
          </InspectorSection>

          <InspectorSection icon={Cpu} title="Agent">
            <InspectorRow label="Type" value={agentType} />
            <InspectorRow
              label="Status"
              value={agentStatus}
              tone={agentStatus === 'ready' ? 'success' : runtimeStatus ? 'default' : 'warning'}
            />
            <InspectorRow label="Provider" value={runtimeStatus?.provider.name ?? provider?.name ?? '—'} />
            <InspectorRow label="Model" value={activeSession?.model ?? runtimeStatus?.provider.model ?? provider?.model ?? '—'} />
          </InspectorSection>

          <InspectorSection icon={Shield} title="Policy">
            <InspectorRow label="Permission" value={settings?.permission_mode ?? '—'} />
            <InspectorRow
              label="Pending"
              value={pendingPermission ? pendingPermission.tool_name : 'none'}
              tone={pendingPermission ? 'warning' : 'default'}
            />
            <InspectorRow
              label="Context"
              value={contextUsage ? `${Math.round(contextUsage.ratio * 100)}%` : '—'}
              tone={contextUsage && contextUsage.ratio > 0.8 ? 'warning' : 'default'}
            />
          </InspectorSection>

          <InspectorSection icon={Network} title="MCP">
            <InspectorRow
              label="Connected"
              value={mcpSummary ? `${mcpSummary.status_counts.connected}/${mcpSummary.enabled_servers}` : '—'}
              tone={mcpSummary && mcpSummary.status_counts.connected > 0 ? 'success' : 'default'}
            />
            <InspectorRow label="Warnings" value={mcpSummary ? String(mcpSummary.warning_count) : '—'} />
            <InspectorRow
              label="Needs Auth"
              value={mcpSummary ? String(mcpSummary.status_counts.needs_auth) : '—'}
              tone={mcpSummary && mcpSummary.status_counts.needs_auth > 0 ? 'warning' : 'default'}
            />
          </InspectorSection>

          <InspectorSection icon={Activity} title="Runtime Events">
            <InspectorRow label="Active tools" value={String(liveToolProgress.length)} />
            <InspectorRow label="Tool results" value={String(liveToolResults.length)} />
            <InspectorRow label="Runtime" value={runtimeStatus ? 'online' : 'offline'} tone={runtimeStatus ? 'success' : 'warning'} />
          </InspectorSection>

          <InspectorSection icon={FolderGit2} title="Workspace">
            <div className="break-all font-mono text-[11px] leading-5 text-rc-text-tertiary">
              {activeProjectPath ? formatSensitivePath(activeProjectPath, privacyMode) : 'No active project'}
            </div>
          </InspectorSection>
        </div>
      </div>
    </aside>
  );
}
