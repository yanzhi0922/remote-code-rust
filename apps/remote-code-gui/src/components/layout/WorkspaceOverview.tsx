import {
  Archive,
  Bot,
  Clock,
  FolderGit2,
  MessageSquarePlus,
  MessageSquareText,
  Network,
  Play,
  Plus,
  Search,
  Sparkles,
  TerminalSquare,
} from 'lucide-react';
import { useMemo, useState } from 'react';
import type { ElementType } from 'react';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';
import { formatSensitivePath, cn } from '../../lib/utils';
import type { SessionSummary } from '../../lib/types';

function ToolbarButton({
  icon: Icon,
  label,
  onClick,
  variant = 'default',
}: {
  icon: ElementType;
  label: string;
  onClick: () => void;
  variant?: 'default' | 'primary';
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'inline-flex h-8 items-center gap-2 rounded-md border px-3 text-xs font-medium transition-colors focus-visible:outline-none',
        variant === 'primary'
          ? 'border-rc-accent-primary bg-rc-accent-primary text-white hover:bg-rc-accent-primary-hover'
          : 'border-rc-border-primary bg-rc-bg-surface text-rc-text-secondary hover:border-rc-border-hover hover:bg-rc-bg-hover hover:text-rc-text-primary',
      )}
    >
      <Icon size={14} className={variant === 'primary' ? 'text-white' : 'text-rc-text-tertiary'} />
      <span>{label}</span>
    </button>
  );
}

function formatAgentName(agentType: string | null | undefined) {
  if (agentType === 'remote_codex') return 'Codex';
  if (agentType === 'remote_roo') return 'Roo';
  return 'Claude';
}

function formatRelativeTime(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diffMinutes = Math.floor((now - then) / 60_000);
  if (diffMinutes < 1) return '刚刚';
  if (diffMinutes < 60) return `${diffMinutes}分钟前`;
  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) return `${diffHours}小时前`;
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 7) return `${diffDays}天前`;
  return iso.slice(0, 10);
}

function StatCard({
  icon: Icon,
  label,
  value,
  sub,
  accent,
}: {
  icon: ElementType;
  label: string;
  value: string | number;
  sub?: string;
  accent?: string;
}) {
  return (
    <div className="rounded-lg border border-rc-border-primary bg-rc-bg-surface p-3 transition-colors hover:border-rc-border-hover">
      <div className="mb-2 flex items-center gap-2 text-xs text-rc-text-tertiary">
        <Icon size={13} className={accent ?? 'text-rc-text-tertiary'} />
        <span>{label}</span>
      </div>
      <div className="text-lg font-semibold text-rc-text-primary">{value}</div>
      {sub && <div className="mt-1 text-[11px] text-rc-text-tertiary">{sub}</div>}
    </div>
  );
}

function SessionCard({
  session,
  privacyMode,
  onSelect,
}: {
  session: SessionSummary;
  privacyMode: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className="group w-full rounded-lg border border-rc-border-primary bg-rc-bg-surface p-3.5 text-left transition-all hover:border-rc-border-hover hover:shadow-sm focus-visible:outline-none"
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium text-rc-text-primary group-hover:text-rc-accent-primary transition-colors">
            {privacyMode ? '会话已隐藏' : session.title}
          </div>
          <div className="mt-2 flex flex-wrap items-center gap-2 text-[11px] text-rc-text-tertiary">
            <span className="inline-flex items-center gap-1 rounded bg-rc-bg-tertiary px-1.5 py-0.5 font-mono">
              <Sparkles size={9} className="text-rc-accent-info" />
              {session.provider_name}
            </span>
            {session.model && (
              <span className="inline-flex items-center gap-1 rounded bg-rc-bg-tertiary px-1.5 py-0.5 font-mono">
                {session.model}
              </span>
            )}
          </div>
        </div>
        <div className="flex flex-col items-end gap-1 text-[11px] text-rc-text-tertiary">
          <span className="flex items-center gap-1">
            <Clock size={10} />
            {formatRelativeTime(session.updated_at)}
          </span>
          <Play size={12} className="text-rc-accent-success opacity-0 transition-opacity group-hover:opacity-100" />
        </div>
      </div>
    </button>
  );
}

type ViewMode = 'grid' | 'list';

export function WorkspaceOverview() {
  const projects = useAppStore((state) => state.projects);
  const sessions = useAppStore((state) => state.sessions);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const pickFolderAndAddProject = useAppStore((state) => state.pickFolderAndAddProject);
  const createSession = useAppStore((state) => state.createSession);
  const selectSession = useAppStore((state) => state.selectSession);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const activeAgentType = useAgentStore((state) => state.activeAgentType);

  const [viewMode, setViewMode] = useState<ViewMode>('grid');
  const [searchQuery, setSearchQuery] = useState('');

  const recentSessions = useMemo(
    () =>
      [...sessions]
        .sort((left, right) => right.updated_at.localeCompare(left.updated_at)),
    [sessions],
  );

  const filteredSessions = useMemo(() => {
    if (!searchQuery.trim()) return recentSessions.slice(0, 12);
    const q = searchQuery.toLowerCase();
    return recentSessions.filter(
      (s) =>
        s.title.toLowerCase().includes(q) ||
        s.provider_name.toLowerCase().includes(q) ||
        (s.model ?? '').toLowerCase().includes(q),
    );
  }, [recentSessions, searchQuery]);

  const activeProject = useMemo(
    () =>
      projects.find((project) => project.path === activeProjectPath) ??
      projects[0] ??
      null,
    [activeProjectPath, projects],
  );

  const mcpSummary = runtimeStatus?.mcp;
  const projectCount = projects.length;
  const sessionCount = sessions.length;

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-rc-bg-chat">
      <div className="flex h-12 shrink-0 items-center justify-between border-b border-rc-border-secondary bg-rc-bg-surface px-4">
        <div className="flex min-w-0 items-center gap-2 text-xs text-rc-text-secondary">
          <TerminalSquare size={14} className="shrink-0 text-rc-text-tertiary" />
          <span className="font-semibold text-rc-text-primary">Workbench</span>
          <span className="text-rc-text-tertiary">/</span>
          <span className="truncate">{activeProject ? activeProject.name : '未打开项目'}</span>
        </div>
        <div className="flex items-center gap-2">
          <ToolbarButton
            icon={FolderGit2}
            label="添加项目"
            onClick={() => {
              void pickFolderAndAddProject();
            }}
          />
          <ToolbarButton
            icon={MessageSquarePlus}
            label="新会话"
            onClick={() => {
              void createSession(undefined, activeProject?.path);
            }}
            variant="primary"
          />
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto px-5 py-8">
        <div className="mx-auto flex w-full max-w-chat flex-col gap-6">
          <section className="rounded-xl border border-rc-border-primary bg-rc-bg-surface shadow-xs">
            <div className="grid divide-y divide-rc-border-secondary md:grid-cols-2 md:divide-x md:divide-y-0">
              <div className="flex items-center gap-3 px-4 py-3">
                <FolderGit2 size={16} className="text-rc-text-tertiary" />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-semibold text-rc-text-primary">{activeProject?.name ?? '未选择项目'}</div>
                  <div className="mt-0.5 truncate text-xs text-rc-text-tertiary">
                    {activeProject ? formatSensitivePath(activeProject.path, privacyMode) : `${projectCount} 个项目`}
                  </div>
                </div>
              </div>
              <div className="grid grid-cols-3 divide-x divide-rc-border-secondary">
                <div className="px-3 py-3">
                  <div className="text-[10px] uppercase text-rc-text-tertiary">Sessions</div>
                  <div className="mt-1 text-sm font-semibold text-rc-text-primary">{sessionCount}</div>
                </div>
                <div className="px-3 py-3">
                  <div className="text-[10px] uppercase text-rc-text-tertiary">Agent</div>
                  <div className="mt-1 truncate text-sm font-semibold text-rc-text-primary">{formatAgentName(activeAgentType)}</div>
                </div>
                <div className="px-3 py-3">
                  <div className="text-[10px] uppercase text-rc-text-tertiary">MCP</div>
                  <div className="mt-1 text-sm font-semibold text-rc-text-primary">
                    {mcpSummary ? `${mcpSummary.status_counts.connected}/${mcpSummary.enabled_servers}` : '—'}
                  </div>
                </div>
              </div>
            </div>
          </section>

          <section>
            <div className="flex items-center justify-between gap-3 mb-3">
              <div className="flex items-center gap-2">
                <h2 className="text-xs font-semibold uppercase tracking-wider text-rc-text-tertiary">最近会话</h2>
                <span className="text-[10px] text-rc-text-tertiary">{filteredSessions.length} 条</span>
              </div>

              <div className="flex items-center gap-2">
                <div className="relative">
                  <Search size={12} className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-rc-text-tertiary" />
                  <input
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder="搜索会话..."
                    className="h-7 w-[180px] rounded-md border border-rc-border-primary bg-rc-bg-tertiary pl-6 pr-2 text-xs text-rc-text-primary outline-none placeholder:text-rc-text-tertiary focus:border-rc-border-focus"
                  />
                </div>
                <div className="flex rounded-md border border-rc-border-primary overflow-hidden">
                  <button
                    onClick={() => setViewMode('grid')}
                    className={cn(
                      'px-2 py-1 text-[10px] font-medium transition-colors',
                      viewMode === 'grid'
                        ? 'bg-rc-bg-active text-rc-text-primary'
                        : 'bg-rc-bg-surface text-rc-text-tertiary hover:text-rc-text-primary',
                    )}
                    title="网格视图"
                  >
                    ▦
                  </button>
                  <button
                    onClick={() => setViewMode('list')}
                    className={cn(
                      'px-2 py-1 text-[10px] font-medium transition-colors',
                      viewMode === 'list'
                        ? 'bg-rc-bg-active text-rc-text-primary'
                        : 'bg-rc-bg-surface text-rc-text-tertiary hover:text-rc-text-primary',
                    )}
                    title="列表视图"
                  >
                    ≡
                  </button>
                </div>
              </div>
            </div>

            {filteredSessions.length > 0 ? (
              viewMode === 'grid' ? (
                <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
                  {filteredSessions.map((session) => (
                    <SessionCard
                      key={session.id}
                      session={session}
                      privacyMode={privacyMode}
                      onSelect={() => {
                        void selectSession(session.id);
                      }}
                    />
                  ))}
                </div>
              ) : (
                <div className="divide-y divide-rc-border-secondary rounded-lg border border-rc-border-primary overflow-hidden">
                  {filteredSessions.map((session) => (
                    <button
                      key={session.id}
                      type="button"
                      onClick={() => {
                        void selectSession(session.id);
                      }}
                      className="flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-rc-bg-hover focus-visible:outline-none"
                    >
                      <Sparkles size={13} className="shrink-0 text-rc-accent-info" />
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-sm font-medium text-rc-text-primary">
                          {privacyMode ? '会话已隐藏' : session.title}
                        </div>
                      </div>
                      <span className="shrink-0 text-[11px] font-mono text-rc-text-tertiary">{session.provider_name}</span>
                      <span className="shrink-0 text-[11px] text-rc-text-tertiary">{formatRelativeTime(session.updated_at)}</span>
                    </button>
                  ))}
                </div>
              )
            ) : (
              <div className="flex min-h-[360px] items-center justify-center rounded-lg border border-dashed border-rc-border-primary bg-rc-bg-elevated">
                <div className="text-center">
                  <div className="text-sm font-medium text-rc-text-secondary">
                    {searchQuery ? '无匹配结果' : '暂无打开的会话'}
                  </div>
                  <div className="mt-4 flex justify-center gap-2">
                    <ToolbarButton
                      icon={FolderGit2}
                      label="添加项目"
                      onClick={() => {
                        void pickFolderAndAddProject();
                      }}
                    />
                    <ToolbarButton
                      icon={MessageSquarePlus}
                      label="新会话"
                      onClick={() => {
                        void createSession(undefined, activeProject?.path);
                      }}
                      variant="primary"
                    />
                  </div>
                </div>
              </div>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
