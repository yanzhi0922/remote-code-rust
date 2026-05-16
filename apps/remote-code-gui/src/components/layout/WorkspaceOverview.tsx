import {
  Activity,
  Bot,
  FolderPlus,
  Gauge,
  HardDrive,
  Eye,
  EyeOff,
  Layers3,
  Plus,
  RefreshCw,
  ShieldCheck,
  ShieldAlert,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { truncateMiddle } from '../../lib/utils';
import { useAgentStore } from '../../stores/useAgentStore';
import { useAppStore } from '../../stores/useAppStore';

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

const PRIVACY_STORAGE_KEY = 'remote-code-gui-workspace-overview-privacy';

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
          <div className="text-[11px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
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
  const [privacyMode, setPrivacyMode] = useState(() => {
    try {
      return window.localStorage.getItem(PRIVACY_STORAGE_KEY) === '1';
    } catch {
      return false;
    }
  });

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
  const refreshSessions = useAppStore((state) => state.refreshSessions);
  const refreshRuntimeStatus = useAppStore((state) => state.refreshRuntimeStatus);
  const pickFolderAndAddProject = useAppStore((state) => state.pickFolderAndAddProject);
  const createSession = useAppStore((state) => state.createSession);
  const activeAgentType = useAgentStore((state) => state.activeAgentType);

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

  useEffect(() => {
    try {
      window.localStorage.setItem(PRIVACY_STORAGE_KEY, privacyMode ? '1' : '0');
    } catch {
      // Ignore storage failures in restricted webviews.
    }
  }, [privacyMode]);

  return (
    <section className="shrink-0 px-6 pb-4 pt-5">
      <div className="mx-auto w-full max-w-[1400px]">
        <div className="mb-4 flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-[linear-gradient(135deg,#2563eb_0%,#0891b2_100%)] text-white shadow-[0_12px_24px_rgba(37,99,235,0.22)]">
                <Bot size={20} />
              </div>
              <div className="min-w-0">
                <h1 className="truncate text-2xl font-semibold text-rc-text-primary">Remote Code</h1>
                <div className="mt-0.5 truncate text-sm text-rc-text-secondary">{activeProjectLabel}</div>
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2 rounded-lg border border-white/80 bg-white/80 p-1.5 shadow-sm backdrop-blur dark:border-rc-border-primary dark:bg-rc-bg-surface/90">
            <button
              type="button"
              title={privacyMode ? '显示敏感信息' : '隐藏敏感信息'}
              aria-pressed={privacyMode}
              onClick={() => setPrivacyMode((current) => !current)}
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

        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
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
            detail={activeAgentType ?? '默认'}
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
            detail={`${providerName} / ${truncateMiddle(modelName, 26)}`}
            tone={permissionTone}
          />
        </div>

        <div className="mt-3 grid gap-3 lg:grid-cols-[1.35fr_0.65fr]">
          <div className="min-w-0 rounded-lg border border-white/80 bg-white/80 px-4 py-3 shadow-sm backdrop-blur dark:border-rc-border-primary dark:bg-rc-bg-surface/90">
            <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-2 text-sm">
              <span className="min-w-0 truncate font-medium text-rc-text-primary">
                {activeSessionTitle}
              </span>
              <span className="truncate text-rc-text-secondary">{providerName}</span>
              <span className="truncate font-mono text-xs text-rc-text-tertiary">{modelName}</span>
            </div>
          </div>
          <div className="flex items-center gap-3 rounded-lg border border-white/80 bg-white/80 px-4 py-3 text-sm shadow-sm backdrop-blur dark:border-rc-border-primary dark:bg-rc-bg-surface/90">
            <HardDrive size={16} className="text-rc-text-tertiary" />
            <span className="min-w-0 truncate text-rc-text-secondary">
              {projects.length} 个项目 · {runtimeStatus ? '本机运行时在线' : '本机运行时离线'}
            </span>
          </div>
        </div>
      </div>
    </section>
  );
}
