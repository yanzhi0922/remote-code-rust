import {
  Activity,
  Cpu,
  Database,
  FolderPlus,
  Gauge,
  Eye,
  EyeOff,
  Layers3,
  Network,
  Plus,
  RefreshCw,
  Server,
  ShieldCheck,
  ShieldAlert,
  Terminal,
  Wifi,
  WifiOff,
} from 'lucide-react';
import { useMemo } from 'react';
import type { AgentType } from '../../lib/types';
import { truncateMiddle } from '../../lib/utils';
import { useAgentStore } from '../../stores/useAgentStore';
import { useAppStore } from '../../stores/useAppStore';
import { BrandMark } from '../brand/BrandMark';

function formatPercent(value: number | null): string {
  if (value === null) return '—';
  return `${Math.round(value * 100)}%`;
}

function permissionLabel(mode: string | null | undefined): string {
  switch (mode) {
    case 'bypassPermissions':
      return '全自动';
    case 'acceptEdits':
      return '自动编辑';
    case 'dontAsk':
      return '低风险自动';
    case 'plan':
      return '规划';
    case 'default':
      return '默认';
    default:
      return mode || '未配置';
  }
}

const AGENT_CARDS: Array<{ type: AgentType; label: string; detail: string }> = [
  { type: 'remote_claude', label: 'Claude', detail: '默认推理与工具执行' },
  { type: 'remote_roo', label: 'Roo', detail: '模式化项目开发' },
  { type: 'remote_codex', label: 'Codex', detail: '原生线程与目标控制' },
];

function agentStatusLabel(status: string | undefined, installed: boolean, available: boolean): string {
  if (!installed) return '未安装';
  if (status) return status;
  return available ? '就绪' : '离线';
}

function MetricTile({
  icon: Icon,
  label,
  value,
  detail,
  tone = 'default',
}: {
  icon: typeof Activity;
  label: string;
  value: string;
  detail: string;
  tone?: 'default' | 'success' | 'warning';
}) {
  const toneClass =
    tone === 'success'
      ? 'bg-rc-accent-success-bg text-rc-accent-success'
      : tone === 'warning'
        ? 'bg-rc-accent-warning-bg text-rc-accent-warning'
        : 'bg-[#e8f4f5] text-[#147181] dark:bg-rc-bg-active dark:text-rc-accent-info';

  return (
    <div className="min-w-0 rounded-lg border border-white/80 bg-white/90 p-3 shadow-sm dark:border-rc-border-primary dark:bg-rc-bg-surface/90">
      <div className="flex items-start gap-3">
        <div className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ${toneClass}`}>
          <Icon size={17} />
        </div>
        <div className="min-w-0">
          <div className="text-[11px] font-semibold text-rc-text-tertiary">
            {label}
          </div>
          <div className="mt-0.5 truncate text-xl font-semibold text-rc-text-primary">{value}</div>
          <div className="mt-0.5 truncate text-xs text-rc-text-secondary">{detail}</div>
        </div>
      </div>
    </div>
  );
}

export function WorkspaceOverview() {
  const provider = useAppStore((state) => state.provider);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const sessions = useAppStore((state) => state.sessions);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const projects = useAppStore((state) => state.projects);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const runningSessionIds = useAppStore((state) => state.runningSessionIds);
  const contextUsageBySession = useAppStore((state) => state.contextUsageBySession);
  const settings = useAppStore((state) => state.settings);
  const lastPromptResult = useAppStore((state) => state.lastPromptResult);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const setPrivacyMode = useAppStore((state) => state.setWorkspacePrivacyMode);
  const refreshSessions = useAppStore((state) => state.refreshSessions);
  const refreshRuntimeStatus = useAppStore((state) => state.refreshRuntimeStatus);
  const pickFolderAndAddProject = useAppStore((state) => state.pickFolderAndAddProject);
  const createSession = useAppStore((state) => state.createSession);
  const activeAgentType = useAgentStore((state) => state.activeAgentType);
  const availableAgents = useAgentStore((state) => state.availableAgents);
  const agentStatuses = useAgentStore((state) => state.agentStatuses);

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );
  const contextUsage = activeSessionId ? contextUsageBySession[activeSessionId] : null;
  const providerName = runtimeStatus?.provider.name ?? provider?.name ?? '未配置';
  const modelName = runtimeStatus?.provider.model ?? provider?.model ?? settings?.provider_model ?? '未配置';
  const permission = permissionLabel(runtimeStatus?.permission_mode ?? settings?.permission_mode);
  const runningCount = runningSessionIds.size;
  const contextRatio = contextUsage?.ratio ?? null;
  const activeProjectLabel = activeProjectPath
    ? privacyMode
      ? '项目路径已隐藏'
      : truncateMiddle(activeProjectPath, 72)
    : '未选择项目';
  const sessionDetail = activeSession
    ? privacyMode
      ? '当前会话已隐藏'
      : truncateMiddle(activeSession.title, 48)
    : `${sessions.length} 个历史会话`;
  const permissionTone = permission === '全自动' ? 'warning' : 'success';
  const activeSessionTitle = privacyMode
    ? '当前会话已隐藏'
    : activeSession?.title ?? '未选择会话';
  const activeAgentKey = activeAgentType ?? 'remote_claude';
  const mcpSummary = runtimeStatus?.mcp ?? null;
  const mcpIssueCount = mcpSummary
    ? mcpSummary.status_counts.failed + mcpSummary.status_counts.needs_auth + mcpSummary.warning_count
    : 0;
  const mcpStatusText = mcpSummary
    ? `${mcpSummary.status_counts.connected}/${mcpSummary.enabled_servers} 已连接`
    : '等待运行时';
  const agentCards = useMemo(
    () =>
      AGENT_CARDS.map((agent) => {
        const info = availableAgents.find((item) => item.agentType === agent.type);
        const installed = info?.installed ?? agent.type === 'remote_claude';
        const available = info?.available ?? agent.type === 'remote_claude';
        const status = agentStatuses[agent.type];
        return {
          ...agent,
          displayName: info?.displayName ?? `Remote ${agent.label}`,
          installed,
          available,
          statusLabel: agentStatusLabel(status, installed, available),
          active: activeAgentKey === agent.type,
        };
      }),
    [activeAgentKey, agentStatuses, availableAgents],
  );

  return (
    <section className="shrink-0 px-5 pb-3 pt-4">
      <div className="mx-auto w-full max-w-[1500px]">
        <div className="mb-3 flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-[#111827] shadow-[0_12px_24px_rgba(37,99,235,0.22)]">
                <BrandMark className="h-7 w-7" />
              </div>
              <div className="min-w-0">
                <div className="flex min-w-0 items-center gap-2">
                  <h1 className="truncate text-2xl font-semibold text-rc-text-primary">Remote Code</h1>
                  <span className="rounded-md bg-rc-accent-primary-light px-2 py-0.5 text-xs font-medium text-rc-accent-primary">
                    Workbench
                  </span>
                </div>
                <div className="mt-0.5 truncate text-sm text-rc-text-secondary">{activeProjectLabel}</div>
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2 rounded-lg border border-white/80 bg-white/80 p-1.5 shadow-sm backdrop-blur dark:border-rc-border-primary dark:bg-rc-bg-surface/90">
            <button
              type="button"
              title={privacyMode ? '显示敏感信息' : '隐藏敏感信息'}
              aria-pressed={privacyMode}
              onClick={() => setPrivacyMode(!privacyMode)}
              className="flex h-9 w-9 items-center justify-center rounded-md text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary active:scale-[0.98]"
            >
              {privacyMode ? <EyeOff size={17} /> : <Eye size={17} />}
            </button>
            <button
              type="button"
              title="添加项目"
              onClick={() => {
                void pickFolderAndAddProject();
              }}
              className="flex h-9 w-9 items-center justify-center rounded-md text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary active:scale-[0.98]"
            >
              <FolderPlus size={17} />
            </button>
            <button
              type="button"
              title="新会话"
              disabled={!activeProjectPath}
              onClick={() => {
                if (activeProjectPath) void createSession(undefined, activeProjectPath);
              }}
              className="flex h-9 w-9 items-center justify-center rounded-md bg-[linear-gradient(135deg,#2563eb_0%,#0891b2_100%)] text-white shadow-sm transition-all hover:shadow-md active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-45"
            >
              <Plus size={17} />
            </button>
            <button
              type="button"
              title="刷新状态"
              onClick={() => {
                void Promise.all([refreshSessions(), refreshRuntimeStatus()]);
              }}
              className="flex h-9 w-9 items-center justify-center rounded-md text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary active:scale-[0.98]"
            >
              <RefreshCw size={16} />
            </button>
          </div>
        </div>

        <div className="grid gap-3 xl:grid-cols-[1.15fr_0.85fr]">
          <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <MetricTile
              icon={Layers3}
              label="会话"
              value={String(sessions.length)}
              detail={sessionDetail}
            />
            <MetricTile
              icon={Activity}
              label="执行"
              value={runningCount > 0 ? `${runningCount} 运行中` : '空闲'}
              detail={agentCards.find((agent) => agent.active)?.displayName ?? 'Remote Claude'}
              tone={runningCount > 0 ? 'warning' : 'success'}
            />
            <MetricTile
              icon={Gauge}
              label="上下文"
              value={formatPercent(contextRatio)}
              detail={lastPromptResult ? `${lastPromptResult.num_turns} 轮` : '等待首轮结果'}
              tone={contextRatio !== null && contextRatio > 0.8 ? 'warning' : 'default'}
            />
            <MetricTile
              icon={permissionTone === 'warning' ? ShieldAlert : ShieldCheck}
              label="权限"
              value={permission}
              detail={`${providerName} / ${truncateMiddle(modelName, 22)}`}
              tone={permissionTone}
            />
          </div>

          <div className="rounded-lg border border-white/80 bg-white/90 p-3 shadow-sm backdrop-blur dark:border-rc-border-primary dark:bg-rc-bg-surface/90">
            <div className="mb-2 flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <Cpu size={16} className="text-rc-text-tertiary" />
                <span className="truncate text-sm font-semibold text-rc-text-primary">Agent 控制台</span>
              </div>
              <span className="text-xs text-rc-text-tertiary">Rust 原生接入</span>
            </div>
            <div className="grid gap-2 sm:grid-cols-3">
              {agentCards.map((agent) => {
                const dotClass = !agent.installed
                  ? 'bg-rc-text-tertiary'
                  : agent.available
                    ? 'bg-rc-accent-success'
                    : 'bg-rc-accent-warning';
                return (
                  <div
                    key={agent.type}
                    className={`min-w-0 rounded-md border px-3 py-2 ${
                      agent.active
                        ? 'border-rc-border-focus bg-rc-bg-selected'
                        : 'border-rc-border-secondary bg-rc-bg-secondary/70'
                    }`}
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      <span className={`h-2 w-2 shrink-0 rounded-full ${dotClass}`} />
                      <span className="truncate text-sm font-semibold text-rc-text-primary">
                        {agent.displayName}
                      </span>
                    </div>
                    <div className="mt-1 truncate text-xs text-rc-text-secondary">{agent.detail}</div>
                    <div className="mt-1 text-xs text-rc-text-tertiary">{agent.statusLabel}</div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        <div className="mt-3 grid gap-3 lg:grid-cols-[1.2fr_0.8fr_0.75fr]">
          <div className="min-w-0 rounded-lg border border-white/80 bg-white/80 px-4 py-3 shadow-sm backdrop-blur dark:border-rc-border-primary dark:bg-rc-bg-surface/90">
            <div className="mb-1 flex items-center gap-2 text-xs font-medium text-rc-text-tertiary">
              <Terminal size={14} />
              当前工作单元
            </div>
            <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-2 text-sm">
              <span className="min-w-0 truncate font-semibold text-rc-text-primary">
                {activeSessionTitle}
              </span>
              <span className="truncate text-rc-text-secondary">{providerName}</span>
              <span className="truncate font-mono text-xs text-rc-text-tertiary">{modelName}</span>
            </div>
          </div>
          <div className="flex min-w-0 items-center gap-3 rounded-lg border border-white/80 bg-white/80 px-4 py-3 text-sm shadow-sm backdrop-blur dark:border-rc-border-primary dark:bg-rc-bg-surface/90">
            {runtimeStatus ? (
              <Wifi size={16} className="text-rc-accent-success" />
            ) : (
              <WifiOff size={16} className="text-rc-text-tertiary" />
            )}
            <span className="min-w-0 truncate text-rc-text-secondary">
              {projects.length} 个项目 · {runtimeStatus ? '本机运行时在线' : '本机运行时离线'}
            </span>
          </div>
          <div className="flex min-w-0 items-center gap-3 rounded-lg border border-white/80 bg-white/80 px-4 py-3 text-sm shadow-sm backdrop-blur dark:border-rc-border-primary dark:bg-rc-bg-surface/90">
            {mcpIssueCount > 0 ? (
              <Network size={16} className="text-rc-accent-warning" />
            ) : runtimeStatus ? (
              <Server size={16} className="text-rc-accent-success" />
            ) : (
              <Database size={16} className="text-rc-text-tertiary" />
            )}
            <span className="min-w-0 truncate text-rc-text-secondary">
              MCP {mcpStatusText}
              {mcpIssueCount > 0 ? ` · ${mcpIssueCount} 个告警` : ''}
            </span>
          </div>
        </div>
      </div>
    </section>
  );
}
