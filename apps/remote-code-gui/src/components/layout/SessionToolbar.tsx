import {
  Bot,
  Boxes,
  FolderTree,
  GitBranch,
  PauseCircle,
  ShieldCheck,
  Sparkles,
  TerminalSquare,
} from 'lucide-react';
import { useMemo } from 'react';
import { normalizePathKey, truncateMiddle } from '../../lib/utils';
import { useAppStore } from '../../stores/useAppStore';

function ToolbarMetric({
  label,
  value,
  tone = 'default',
}: {
  label: string;
  value: string;
  tone?: 'default' | 'success' | 'warning' | 'error';
}) {
  const toneClass =
    tone === 'success'
      ? 'border-rc-accent-success bg-rc-accent-success-bg text-rc-accent-success'
      : tone === 'warning'
        ? 'border-rc-accent-warning bg-rc-accent-warning-bg text-rc-accent-warning'
        : tone === 'error'
          ? 'border-rc-accent-error bg-rc-accent-error-bg text-rc-accent-error'
          : 'border-rc-border-primary bg-rc-bg-secondary text-rc-text-secondary';

  return (
    <div className={`inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs ${toneClass}`}>
      <span className="text-rc-text-tertiary">{label}</span>
      <span className="font-medium">{value}</span>
    </div>
  );
}

export function SessionToolbar() {
  const provider = useAppStore((state) => state.provider);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const settings = useAppStore((state) => state.settings);
  const sessions = useAppStore((state) => state.sessions);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const projects = useAppStore((state) => state.projects);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const contextUsageBySession = useAppStore((state) => state.contextUsageBySession);
  const contextOverflowBySession = useAppStore((state) => state.contextOverflowBySession);
  const contextCompactionBySession = useAppStore((state) => state.contextCompactionBySession);
  const runningSessionIds = useAppStore((state) => state.runningSessionIds);
  const cancelPrompt = useAppStore((state) => state.cancelPrompt);

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );

  const activeProject = useMemo(
    () =>
      (activeSession &&
        projects.find((project) => normalizePathKey(project.path) === normalizePathKey(activeSession.cwd))) ??
      projects.find(
        (project) =>
          activeProjectPath && normalizePathKey(project.path) === normalizePathKey(activeProjectPath),
      ) ??
      null,
    [activeProjectPath, activeSession, projects],
  );

  const modelLabel =
    activeSession?.model ??
    runtimeStatus?.provider.model ??
    settings?.provider_model ??
    provider?.model ??
    '未配置';
  const providerLabel =
    activeSession?.provider_name ?? runtimeStatus?.provider.name ?? settings?.provider_name ?? provider?.name ?? '未连接';
  const permissionLabel = runtimeStatus?.permission_mode ?? settings?.permission_mode ?? 'default';
  const usage = activeSessionId ? contextUsageBySession[activeSessionId] ?? null : null;
  const overflow = activeSessionId ? contextOverflowBySession[activeSessionId] ?? null : null;
  const compaction = activeSessionId ? contextCompactionBySession[activeSessionId] ?? null : null;
  const isRunning = !!activeSessionId && runningSessionIds.has(activeSessionId);
  const cwd = activeSession?.cwd ?? activeProjectPath ?? '';

  return (
    <header className="flex h-header shrink-0 items-center gap-3 border-b border-rc-border-primary bg-rc-bg-elevated px-4">
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-rc-bg-active text-rc-accent-primary">
          <TerminalSquare size={17} />
        </div>
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-2">
            <div className="truncate text-sm font-semibold text-rc-text-primary">
              {activeSession?.title || activeProject?.name || 'Remote Code'}
            </div>
            {isRunning && (
              <span className="inline-flex items-center gap-1 rounded-md bg-rc-accent-warning-bg px-1.5 py-0.5 text-[11px] font-medium text-rc-accent-warning">
                <span className="h-1.5 w-1.5 rounded-full bg-rc-accent-warning" />
                running
              </span>
            )}
          </div>
          <div className="mt-0.5 flex min-w-0 items-center gap-2 text-xs text-rc-text-tertiary">
            <FolderTree size={12} />
            <span className="truncate">{cwd ? truncateMiddle(cwd, 86) : '选择项目后开始'}</span>
          </div>
        </div>
      </div>

      <div className="hidden min-w-0 items-center gap-2 xl:flex">
        <ToolbarMetric label="provider" value={providerLabel} />
        <ToolbarMetric label="model" value={modelLabel} />
        <ToolbarMetric label="mode" value={permissionLabel} />
        {runtimeStatus?.mcp && runtimeStatus.mcp.total_servers > 0 && (
          <ToolbarMetric label="mcp" value={`${runtimeStatus.mcp.enabled_servers}/${runtimeStatus.mcp.total_servers}`} />
        )}
        {usage && (
          <ToolbarMetric
            label="ctx"
            value={`${Math.round(usage.ratio * 100)}%`}
            tone={usage.ratio > 0.8 ? 'warning' : 'default'}
          />
        )}
        {compaction && (
          <ToolbarMetric label="compact" value={`${compaction.entries_removed}`} tone="warning" />
        )}
        {!compaction && overflow && (
          <ToolbarMetric label="limit" value={`${Math.round(overflow.ratio * 100)}%`} tone="error" />
        )}
      </div>

      <div className="flex shrink-0 items-center gap-1">
        <div className="hidden items-center gap-1 text-xs text-rc-text-tertiary lg:flex">
          <Bot size={14} />
          <span>{runtimeStatus?.provider.effort ?? 'medium'}</span>
        </div>
        <div className="hidden items-center gap-1 text-xs text-rc-text-tertiary lg:flex">
          <ShieldCheck size={14} />
          <span>{permissionLabel}</span>
        </div>
        <div className="hidden items-center gap-1 text-xs text-rc-text-tertiary lg:flex">
          <Boxes size={14} />
          <span>{runtimeStatus?.mcp?.enabled_servers ?? 0}</span>
        </div>
        <div className="hidden items-center gap-1 text-xs text-rc-text-tertiary lg:flex">
          <GitBranch size={14} />
          <span>worktree</span>
        </div>
        {isRunning && activeSessionId ? (
          <button
            type="button"
            className="workbench-button border-rc-accent-warning bg-rc-accent-warning-bg text-rc-accent-warning"
            onClick={() => {
              void cancelPrompt(activeSessionId);
            }}
          >
            <PauseCircle size={15} />
            Stop
          </button>
        ) : (
          <div className="hidden items-center gap-1 text-xs text-rc-text-tertiary md:flex">
            <Sparkles size={14} />
            <span>ready</span>
          </div>
        )}
      </div>
    </header>
  );
}
