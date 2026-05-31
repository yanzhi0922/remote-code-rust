import {
  Archive,
  Clock,
  FolderGit2,
  LayoutGrid,
  List,
  MessageSquarePlus,
  Play,
  Plus,
  Search,
  Sparkles,
  TerminalSquare,
  Pin,
  PinOff,
  EyeOff,
  Eye,
  Pencil,
  Trash2,
  ExternalLink,
  Copy,
  FileText,
  RefreshCw,
  MessageCircleWarning,
  ChevronRight,
  FolderOpen,
} from 'lucide-react';
import { useCallback, useMemo, useState } from 'react';
import type { ElementType } from 'react';
import { useTranslation } from 'react-i18next';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';
import { formatSensitivePath, cn } from '../../lib/utils';
import type { SessionSummary } from '../../lib/types';
import { useContextMenu, type ContextMenuItem } from '../shared/ContextMenu';
import { FileExplorer } from '../shared/FileExplorer';
import { AgentSelector } from '../agent/AgentSelector';

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

function formatRelativeTime(iso: string, t: (key: string, options?: { count?: number }) => string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diffMinutes = Math.floor((now - then) / 60_000);
  if (diffMinutes < 1) return t('time.justNow');
  if (diffMinutes < 60) return t('time.minutesAgo', { count: diffMinutes });
  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) return t('time.hoursAgo', { count: diffHours });
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 7) return t('time.daysAgo', { count: diffDays });
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

/** Inline rename input for session titles */
function RenameInput({
  initialValue,
  onConfirm,
  onCancel,
  t,
}: {
  initialValue: string;
  onConfirm: (val: string) => void;
  onCancel: () => void;
  t: (key: string) => string;
}) {
  const [val, setVal] = useState(initialValue);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') onConfirm(val.trim());
    if (e.key === 'Escape') onCancel();
  };

  return (
    <input
      autoFocus
      value={val}
      onChange={(e) => setVal(e.target.value)}
      onBlur={() => onConfirm(val.trim())}
      onKeyDown={handleKeyDown}
      className="w-full rounded border border-rc-border-focus bg-rc-bg-tertiary px-2 py-0.5 text-sm text-rc-text-primary outline-none"
      placeholder={t('contextMenu.renamePlaceholder')}
    />
  );
}

function SessionCard({
  session,
  privacyMode,
  onSelect,
  t,
  pinned,
  unread,
  renaming,
  onContextMenu,
  onRenameDone,
}: {
  session: SessionSummary;
  privacyMode: boolean;
  onSelect: () => void;
  t: (key: string) => string;
  pinned: boolean;
  unread: boolean;
  renaming: boolean;
  onContextMenu: (e: React.MouseEvent) => void;
  onRenameDone: () => void;
}) {

  return (
    <div
      className="group relative w-full"
      onContextMenu={onContextMenu}
    >
      <button
        type="button"
        onClick={onSelect}
        className="w-full rounded-lg border border-rc-border-primary bg-rc-bg-surface p-3.5 text-left transition-all hover:border-rc-border-hover hover:shadow-sm focus-visible:outline-none"
      >
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              {pinned && <Pin size={11} className="shrink-0 text-rc-accent-primary" />}
              {unread && <span className="h-2 w-2 shrink-0 rounded-full bg-rc-accent-info" />}
              {renaming ? (
                <RenameInput
                  initialValue={session.title}
                  onConfirm={(v) => {
                    onRenameDone();
                    if (v) void useAppStore.getState().renameSession(session.id, v);
                  }}
                  onCancel={() => onRenameDone()}
                  t={t}
                />
              ) : (
                <div className="truncate text-sm font-medium text-rc-text-primary group-hover:text-rc-accent-primary transition-colors">
                  {privacyMode ? t('settings.sessionArchived') : session.title}
                </div>
              )}
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
              {formatRelativeTime(session.updated_at, t)}
            </span>
            <Play size={12} className="text-rc-accent-success opacity-0 transition-opacity group-hover:opacity-100" />
          </div>
        </div>
      </button>
    </div>
  );
}

type ViewMode = 'grid' | 'list';

export function WorkspaceOverview() {
  const { t } = useTranslation();
  const projects = useAppStore((state) => state.projects);
  const sessions = useAppStore((state) => state.sessions);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const pickFolderAndAddProject = useAppStore((state) => state.pickFolderAndAddProject);
  const createSession = useAppStore((state) => state.createSession);
  const selectSession = useAppStore((state) => state.selectSession);
  const archiveSession = useAppStore((state) => state.archiveSession);
  const renameSession = useAppStore((state) => state.renameSession);
  const togglePinSession = useAppStore((state) => state.togglePinSession);
  const toggleUnreadSession = useAppStore((state) => state.toggleUnreadSession);
  const pinnedSessions = useAppStore((state) => state.pinnedSessions);
  const unreadSessions = useAppStore((state) => state.unreadSessions);
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const activeAgentType = useAgentStore((state) => state.activeAgentType);
  const availableAgents = useAgentStore((state) => state.availableAgents);
  const selectAgent = useAgentStore((state) => state.selectAgent);
  const fileExplorerPath = useAppStore((state) => state.fileExplorerPath);
  const fileExplorerProjectName = useAppStore((state) => state.fileExplorerProjectName);
  const closeFileExplorer = useAppStore((state) => state.closeFileExplorer);

  const [viewMode, setViewMode] = useState<ViewMode>('grid');
  const [searchQuery, setSearchQuery] = useState('');
  const [projectHoverPath, setProjectHoverPath] = useState<string | null>(null);
  const [renamingSessionId, setRenamingSessionId] = useState<string | null>(null);

  // Global context menu for workspace sessions
  const { show: showMenu, MenuComponent } = useContextMenu();

  const recentSessions = useMemo(
    () =>
      [...sessions]
        .sort((left, right) => {
          // Pinned sessions first, then by updated_at
          const leftPinned = pinnedSessions.has(left.id) ? 1 : 0;
          const rightPinned = pinnedSessions.has(right.id) ? 1 : 0;
          if (leftPinned !== rightPinned) return rightPinned - leftPinned;
          return right.updated_at.localeCompare(left.updated_at);
        }),
    [sessions, pinnedSessions],
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

  /** Build context menu items for a session */
  const buildSessionMenu = useCallback(
    (session: SessionSummary): ContextMenuItem[] => {
      const isPinned = pinnedSessions.has(session.id);
      const isUnread = unreadSessions.has(session.id);
      return [
        {
          key: 'pin',
          label: isPinned ? t('contextMenu.unpinTask') : t('contextMenu.pinTask'),
          icon: isPinned ? <PinOff size={13} /> : <Pin size={13} />,
          action: () => togglePinSession(session.id),
        },
        {
          key: 'rename',
          label: t('contextMenu.renameTask'),
          icon: <Pencil size={13} />,
          action: () => setRenamingSessionId(session.id),
        },
        {
          key: 'archive',
          label: t('contextMenu.archiveTask'),
          icon: <Archive size={13} />,
          danger: true,
          action: () => void archiveSession(session.id),
        },
        {
          key: 'unread',
          label: isUnread ? t('contextMenu.markRead') : t('contextMenu.markUnread'),
          icon: isUnread ? <Eye size={13} /> : <EyeOff size={13} />,
          action: () => toggleUnreadSession(session.id),
        },
        { key: 'sep1', label: '', separator: true, action: () => {} },
        {
          key: 'open-explorer',
          label: t('contextMenu.openInExplorer'),
          icon: <ExternalLink size={13} />,
          action: () => {
            revealItemInDir(session.cwd).catch(() => {
              navigator.clipboard.writeText(session.cwd).catch(() => {});
            });
          },
        },
        {
          key: 'copy-path',
          label: t('contextMenu.copyPath'),
          icon: <Copy size={13} />,
          action: () => {
            navigator.clipboard.writeText(session.cwd).catch(() => {});
          },
        },
        {
          key: 'copy-session-id',
          label: t('contextMenu.copySessionId'),
          icon: <FileText size={13} />,
          action: () => {
            navigator.clipboard.writeText(session.id).catch(() => {});
          },
        },
        { key: 'sep2', label: '', separator: true, action: () => {} },
        {
          key: 'reload',
          label: t('contextMenu.reloadSession'),
          icon: <RefreshCw size={13} />,
          action: () => void selectSession(session.id),
        },
        {
          key: 'feedback',
          label: t('contextMenu.feedbackIssue'),
          icon: <MessageCircleWarning size={13} />,
          action: () => {
            window.open('https://github.com/yanzhi0922/remote-code/issues', '_blank')?.focus();
          },
        },
      ];
    },
    [t, pinnedSessions, unreadSessions, togglePinSession, toggleUnreadSession, archiveSession, selectSession],
  );

  // File explorer mode — show file tree for selected project
  if (fileExplorerPath && fileExplorerProjectName) {
    return (
      <div className="relative flex min-h-0 flex-1 flex-col">
        <FileExplorer
          rootPath={fileExplorerPath}
          projectName={fileExplorerProjectName}
          onBack={() => closeFileExplorer()}
          onAddToChat={(path) => useAppStore.getState().injectChatAttachment(path)}
        />
        {MenuComponent}
      </div>
    );
  }

  return (
    <div className="relative flex min-h-0 flex-1 flex-col bg-rc-bg-chat">
      {/* Project hover popover */}
      {projectHoverPath && activeProject && activeProject.path === projectHoverPath && (
        <div
          className="absolute z-30 mt-1 rounded-lg border border-rc-border-primary bg-rc-bg-surface py-1 shadow-lg"
          style={{ marginLeft: '180px' }}
          onMouseLeave={() => setProjectHoverPath(null)}
        >
          <button
            type="button"
            onClick={() => {
              void useAppStore.getState().removeProject(projectHoverPath);
              setProjectHoverPath(null);
            }}
            className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-accent-error"
          >
            <Trash2 size={13} />
            {t('projectHover.remove')}
          </button>
          <button
            type="button"
            onClick={() => {
              useAppStore.getState().openFileExplorer(projectHoverPath!, activeProject!.name);
              setProjectHoverPath(null);
            }}
            className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary"
          >
            <FolderOpen size={13} />
            {t('projectHover.viewFiles')}
          </button>
          <button
            type="button"
            onClick={() => {
              void createSession(undefined, projectHoverPath);
              setProjectHoverPath(null);
            }}
            className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary"
          >
            <Plus size={13} />
            {t('projectHover.newTask')}
          </button>
        </div>
      )}

      <div className="flex h-12 shrink-0 items-center justify-between border-b border-rc-border-secondary bg-rc-bg-surface px-4">
        <div
          className="relative flex min-w-0 items-center gap-2 text-xs text-rc-text-secondary"
          onMouseEnter={() => activeProject && setProjectHoverPath(activeProject.path)}
          onMouseLeave={() => setProjectHoverPath(null)}
        >
          <TerminalSquare size={14} className="shrink-0 text-rc-text-tertiary" />
          <span className="font-semibold text-rc-text-primary">Workbench</span>
          <span className="text-rc-text-tertiary">/</span>
          <span className="truncate cursor-default">{activeProject ? activeProject.name : t('workspace.noOpenProject')}</span>
          {activeProject && (
            <ChevronRight size={10} className="text-rc-text-tertiary" />
          )}
        </div>
        <div className="flex items-center gap-2">
          <AgentSelector
            availableAgents={availableAgents}
            activeAgentType={activeAgentType}
            onSelect={(agentType) => { if (agentType) selectAgent(agentType); }}
          />
          <ToolbarButton
            icon={FolderGit2}
            label={t('workspace.addProjectBtn')}
            onClick={() => {
              void pickFolderAndAddProject();
            }}
          />
          <ToolbarButton
            icon={MessageSquarePlus}
            label={t('workspace.newSessionBtn')}
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
                  <div className="text-[10px] uppercase text-rc-text-tertiary">{t('workspace.currentProject')}</div>
                  <div className="truncate text-sm font-semibold text-rc-text-primary">{activeProject?.name ?? t('workspace.noProjectSelected')}</div>
                  <div className="mt-0.5 truncate text-xs text-rc-text-tertiary">
                    {activeProject ? formatSensitivePath(activeProject.path, privacyMode) : t('workspace.projectCount', { count: projectCount })}
                  </div>
                </div>
              </div>
              <div className="grid grid-cols-3 divide-x divide-rc-border-secondary">
                <div className="px-3 py-3">
                  <div className="text-[10px] uppercase text-rc-text-tertiary">{t('workspace.sessionsLabel')}</div>
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
                <h2 className="text-xs font-semibold uppercase tracking-wider text-rc-text-tertiary">{t('workspace.recentSessions')}</h2>
                <span className="text-[10px] text-rc-text-tertiary">{t('workspace.sessionCount', { count: filteredSessions.length })}</span>
              </div>

              <div className="flex items-center gap-2">
                <div className="relative">
                  <Search size={12} className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-rc-text-tertiary" />
                  <input
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder={t('workspace.searchSessions')}
                    className="h-7 w-[180px] rounded-md border border-rc-border-primary bg-rc-bg-tertiary pl-6 pr-2 text-xs text-rc-text-primary outline-none placeholder:text-rc-text-tertiary focus:border-rc-border-focus"
                  />
                </div>
                <div className="flex rounded-md border border-rc-border-primary overflow-hidden">
                  <button
                    onClick={() => setViewMode('grid')}
                    className={cn(
                      'flex items-center px-2 py-1 text-[10px] font-medium transition-colors',
                      viewMode === 'grid'
                        ? 'bg-rc-bg-active text-rc-text-primary'
                        : 'bg-rc-bg-surface text-rc-text-tertiary hover:text-rc-text-primary',
                    )}
                    title={t('workspace.gridView')}
                  >
                    <LayoutGrid size={12} />
                  </button>
                  <button
                    onClick={() => setViewMode('list')}
                    className={cn(
                      'flex items-center px-2 py-1 text-[10px] font-medium transition-colors',
                      viewMode === 'list'
                        ? 'bg-rc-bg-active text-rc-text-primary'
                        : 'bg-rc-bg-surface text-rc-text-tertiary hover:text-rc-text-primary',
                    )}
                    title={t('workspace.listView')}
                  >
                    <List size={12} />
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
                      t={t}
                      pinned={pinnedSessions.has(session.id)}
                      unread={unreadSessions.has(session.id)}
                      renaming={renamingSessionId === session.id}
                      onContextMenu={(e) => showMenu(e, buildSessionMenu(session))}
                      onRenameDone={() => setRenamingSessionId(null)}
                    />
                  ))}
                </div>
              ) : (
                <div className="divide-y divide-rc-border-secondary rounded-lg border border-rc-border-primary overflow-hidden">
                  {filteredSessions.map((session) => (
                    <div
                      key={session.id}
                      className="group"
                      onContextMenu={(e) => showMenu(e, buildSessionMenu(session))}
                    >
                      <button
                        type="button"
                        onClick={() => {
                          void selectSession(session.id);
                        }}
                        className="flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-rc-bg-hover focus-visible:outline-none"
                      >
                        {pinnedSessions.has(session.id) && <Pin size={11} className="shrink-0 text-rc-accent-primary" />}
                        {unreadSessions.has(session.id) && <span className="h-2 w-2 shrink-0 rounded-full bg-rc-accent-info" />}
                        <Sparkles size={13} className="shrink-0 text-rc-accent-info" />
                        <div className="min-w-0 flex-1">
                          <div className="truncate text-sm font-medium text-rc-text-primary">
                            {privacyMode ? t('settings.sessionArchived') : session.title}
                          </div>
                        </div>
                        <span className="shrink-0 text-[11px] font-mono text-rc-text-tertiary">{session.provider_name}</span>
                        <span className="shrink-0 text-[11px] text-rc-text-tertiary">{formatRelativeTime(session.updated_at, t)}</span>
                      </button>
                    </div>
                  ))}
                </div>
              )
            ) : (
              <div className="flex min-h-[360px] items-center justify-center rounded-lg border border-dashed border-rc-border-primary bg-rc-bg-elevated">
                <div className="text-center">
                  <div className="text-sm font-medium text-rc-text-secondary">
                    {searchQuery ? t('workspace.noMatchResults') : t('workspace.noOpenSessions')}
                  </div>
                  <div className="mt-4 flex justify-center gap-2">
                    <ToolbarButton
                      icon={FolderGit2}
                      label={t('workspace.addProjectBtn')}
                      onClick={() => {
                        void pickFolderAndAddProject();
                      }}
                    />
                    <ToolbarButton
                      icon={MessageSquarePlus}
                      label={t('workspace.newSessionBtn')}
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

      {MenuComponent}
    </div>
  );
}
