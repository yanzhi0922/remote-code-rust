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

function formatAgentName(agentType: string) {
  if (agentType === 'remote_codex') return 'Codex';
  if (agentType === 'remote_roo') return 'Roo';
  return 'Claude';
}

function formatAgentStatus(status: string) {
  if (status === 'ready') return '就绪';
  if (status === 'running') return '运行中';
  if (status === 'offline') return '离线';
  return status;
}

function formatPermissionMode(mode: string | undefined) {
  if (mode === 'acceptEdits') return '自动编辑';
  if (mode === 'bypassPermissions') return '全自动';
  if (mode === 'dontAsk') return '不询问';
  if (mode === 'plan') return '规划';
  if (mode === 'default') return '默认';
  return mode ?? '—';
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
      className="hidden w-[328px] shrink-0 border-l border-rc-border-secondary bg-rc-bg-chat p-3 xl:flex xl:flex-col"
    >
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md border border-rc-border-primary bg-rc-bg-surface shadow-sm">
        <div className="flex h-11 shrink-0 items-center justify-between border-b border-rc-border-secondary px-4">
          <div className="text-sm font-semibold text-rc-text-primary">环境信息</div>
          <Settings2 size={16} className="text-rc-text-tertiary" />
        </div>

        <div className="min-h-0 flex-1 overflow-auto">
        <InspectorSection icon={TerminalSquare} title="会话">
          <InspectorRow label="标题" value={activeSession ? (privacyMode ? '会话已隐藏' : activeSession.title) : '—'} />
          <InspectorRow label="ID" value={activeSessionId ? activeSessionId.slice(0, 8) : '—'} />
          <InspectorRow label="项目" value={activeProjectPath ? formatSensitivePath(activeProjectPath, privacyMode) : '—'} />
        </InspectorSection>

        <InspectorSection icon={Cpu} title="Agent">
          <InspectorRow label="类型" value={formatAgentName(agentType)} />
          <InspectorRow
            label="状态"
            value={formatAgentStatus(agentStatus)}
            tone={agentStatus === 'ready' ? 'success' : runtimeStatus ? 'default' : 'warning'}
          />
          <InspectorRow label="Provider" value={runtimeStatus?.provider.name ?? provider?.name ?? '—'} />
          <InspectorRow label="模型" value={activeSession?.model ?? runtimeStatus?.provider.model ?? provider?.model ?? '—'} />
        </InspectorSection>

        <InspectorSection icon={Shield} title="权限">
          <InspectorRow label="模式" value={formatPermissionMode(settings?.permission_mode)} />
          <InspectorRow
            label="待确认"
            value={pendingPermission ? pendingPermission.tool_name : '无'}
            tone={pendingPermission ? 'warning' : 'default'}
          />
          <InspectorRow
            label="上下文"
            value={contextUsage ? `${Math.round(contextUsage.ratio * 100)}%` : '—'}
            tone={contextUsage && contextUsage.ratio > 0.8 ? 'warning' : 'default'}
          />
        </InspectorSection>

        <InspectorSection icon={Network} title="MCP">
          <InspectorRow
            label="已连接"
            value={mcpSummary ? `${mcpSummary.status_counts.connected}/${mcpSummary.enabled_servers}` : '—'}
            tone={mcpSummary && mcpSummary.status_counts.connected > 0 ? 'success' : 'default'}
          />
          <InspectorRow label="警告" value={mcpSummary ? String(mcpSummary.warning_count) : '—'} />
          <InspectorRow
            label="需认证"
            value={mcpSummary ? String(mcpSummary.status_counts.needs_auth) : '—'}
            tone={mcpSummary && mcpSummary.status_counts.needs_auth > 0 ? 'warning' : 'default'}
          />
        </InspectorSection>

        <InspectorSection icon={Activity} title="运行事件">
          <InspectorRow label="活动工具" value={String(liveToolProgress.length)} />
          <InspectorRow label="工具结果" value={String(liveToolResults.length)} />
          <InspectorRow label="运行时" value={runtimeStatus ? '在线' : '离线'} tone={runtimeStatus ? 'success' : 'warning'} />
        </InspectorSection>

        <InspectorSection icon={FolderGit2} title="工作区">
          <div className="break-all font-mono text-[11px] leading-5 text-rc-text-tertiary">
            {activeProjectPath ? formatSensitivePath(activeProjectPath, privacyMode) : '未选择项目'}
          </div>
        </InspectorSection>
        </div>
      </div>
    </aside>
  );
}
