import {
  Archive,
  ChevronRight,
  Clock,
  Copy,
  Eye,
  EyeOff,
  ExternalLink,
  FileText,
  Folder,
  FolderOpen,
  MessageCircleWarning,
  Pencil,
  Pin,
  PinOff,
  Plus,
  RefreshCw,
  Search,
  SlidersHorizontal,
  Trash2,
  X,
  FolderPlus,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ConversationEntry, SessionSubtask, SessionSummary, ToolCallInfo } from '../../lib/types';
import { cn, normalizePathKey, truncateMiddle } from '../../lib/utils';
import { useDebouncedValue } from '../../lib/hooks';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';
import { useContextMenu, type ContextMenuItem } from '../shared/ContextMenu';
import i18n from '../../i18n';

type SessionTaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'stopped';

interface SessionTaskItem {
  id: string;
  title: string;
  status: SessionTaskStatus;
  detail: string;
  depth: number;
}

// ── Time bucketing (from CodexMonitor pattern) ────────────────

type TimeBucket = 'now' | 'today' | 'yesterday' | 'week' | 'older';

function getTimeBucket(iso: string): TimeBucket {
  const now = new Date();
  const then = new Date(iso);
  const diffMs = now.getTime() - then.getTime();
  const diffMinutes = Math.floor(diffMs / 60_000);
  if (diffMinutes < 30) return 'now';
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  if (then.getTime() >= startOfToday) return 'today';
  if (then.getTime() >= startOfToday - 86_400_000) return 'yesterday';
  if (then.getTime() >= startOfToday - 7 * 86_400_000) return 'week';
  return 'older';
}

const BUCKET_ORDER: TimeBucket[] = ['now', 'today', 'yesterday', 'week', 'older'];
function getBucketLabel(bucket: TimeBucket): string {
  const t = i18n.t.bind(i18n);
  switch (bucket) {
    case 'now': return t('time.justNow');
    case 'today': return t('time.today');
    case 'yesterday': return t('time.yesterday');
    case 'week': return t('time.thisWeek');
    case 'older': return t('time.earlier');
  }
}

function formatRelativeTime(iso: string): string {
  const t = i18n.t.bind(i18n);
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diffMinutes = Math.floor((now - then) / 60_000);
  if (diffMinutes < 1) return t('time.justNow');
  if (diffMinutes < 60) return t('time.minutesAgo', { count: diffMinutes });
  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) return t('time.hoursAgo', { count: diffHours });
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 7) return t('time.daysAgo', { count: diffDays });
  return new Date(iso).toLocaleDateString();
}

// ── Search highlighting (from CodexMonitor pattern) ───────────

function HighlightedText({ text, query }: { text: string; query: string }) {
  if (!query.trim()) return <>{text}</>;

  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const idx = lowerText.indexOf(lowerQuery);
  if (idx === -1) return <>{text}</>;

  return (
    <>
      {text.slice(0, idx)}
      <mark className="search-highlight">{text.slice(idx, idx + query.length)}</mark>
      {text.slice(idx + query.length)}
    </>
  );
}

function parseJson(value: unknown): Record<string, unknown> | null {
  if (typeof value === 'object' && value !== null) {
    return value as Record<string, unknown>;
  }
  if (typeof value !== 'string') return null;
  try {
    const parsed = JSON.parse(value);
    return parsed && typeof parsed === 'object' ? (parsed as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

function summarizeAgentPrompt(toolCall: ToolCallInfo): string {
  const input = parseJson(toolCall.input);
  const prompt = input?.prompt;
  if (typeof prompt === 'string' && prompt.trim()) {
    return truncateMiddle(prompt.trim(), 56);
  }
  return i18n.t('chatArea.subtask');
}

function summarizeAgentOutput(text: string): string {
  const parsed = parseJson(text);
  if (parsed?.type === 'batch_delegation_result') {
    const total = typeof parsed.total === 'number' ? parsed.total : null;
    const succeeded = typeof parsed.succeeded === 'number' ? parsed.succeeded : null;
    if (total !== null && succeeded !== null) {
      return i18n.t('chatArea.batchCompleted', { succeeded, total });
    }
  }
  const compact = text.replace(/\s+/g, ' ').trim();
  return compact ? truncateMiddle(compact, 56) : i18n.t('chatArea.completed');
}

function deriveAgentTasks(conversation: ConversationEntry[]): SessionTaskItem[] {
  const tasks: SessionTaskItem[] = [];
  const taskIndexesByCall = new Map<string, number[]>();

  for (const entry of conversation) {
    if (entry.role !== 'assistant') continue;

    for (const toolCall of entry.tool_calls) {
      if (toolCall.name !== 'agent') continue;

      const input = parseJson(toolCall.input);
      const batchTasks = Array.isArray(input?.tasks)
        ? input?.tasks.filter((task): task is string => typeof task === 'string' && task.trim().length > 0)
        : [];

      if (batchTasks.length > 0) {
        const indexes: number[] = [];
        batchTasks.forEach((task, index) => {
          indexes.push(tasks.length);
          tasks.push({
            id: `${toolCall.id}:${index}`,
            title: truncateMiddle(task, 56),
            status: 'running',
            detail: i18n.t('chatArea.waitingSubagent'),
            depth: 0,
          });
        });
        taskIndexesByCall.set(toolCall.id, indexes);
        continue;
      }

      taskIndexesByCall.set(toolCall.id, [tasks.length]);
      tasks.push({
        id: toolCall.id,
        title: summarizeAgentPrompt(toolCall),
        status: 'running',
        detail: i18n.t('sidebar.waitingSubagent'),
        depth: 0,
      });
    }
  }

  for (const entry of conversation) {
    if (entry.role !== 'tool' || entry.name !== 'agent' || !entry.tool_call_id) continue;
    const indexes = taskIndexesByCall.get(entry.tool_call_id);
    if (!indexes?.length) continue;

    const parsed = parseJson(entry.text);
    const results = Array.isArray(parsed?.results)
      ? parsed.results.filter((result): result is Record<string, unknown> => !!result && typeof result === 'object')
      : [];

    indexes.forEach((taskIndex, index) => {
      const task = tasks[taskIndex];
      if (!task) return;

      const batchResult = results[index];
      const success =
        typeof batchResult?.success === 'boolean'
          ? batchResult.success
          : !(entry.is_error || entry.text.startsWith('Sub-agent failed'));
      const preview =
        typeof batchResult?.output_preview === 'string'
          ? batchResult.output_preview
          : summarizeAgentOutput(entry.text);

      task.status = success ? 'completed' : 'failed';
      task.detail = preview;
    });
  }

  return tasks;
}

function flattenLiveTasks(tasks: SessionSubtask[]): SessionTaskItem[] {
  const byParent = new Map<string | null, SessionSubtask[]>();
  tasks.forEach((task) => {
    const key = task.parent_task_id ?? null;
    const bucket = byParent.get(key) ?? [];
    bucket.push(task);
    byParent.set(key, bucket);
  });

  const visited = new Set<string>();
  const flattened: SessionTaskItem[] = [];

  const visit = (task: SessionSubtask) => {
    if (visited.has(task.task_id)) return;
    visited.add(task.task_id);
    flattened.push({
      id: task.task_id,
      title: truncateMiddle(task.description, 56),
      status: task.status,
      detail: truncateMiddle(task.output_preview ?? task.summary ?? i18n.t('chatArea.waitingSubagent'), 56),
      depth: task.depth,
    });
    const children = byParent.get(task.task_id) ?? [];
    children.forEach(visit);
  };

  const roots = tasks.filter((task) => !task.parent_task_id);
  roots.forEach(visit);
  tasks.forEach(visit);
  return flattened;
}

function StatusDot({ status }: { status: SessionTaskStatus }) {
  const styles = {
    pending: 'bg-rc-text-tertiary',
    stopped: 'bg-rc-text-tertiary',
    completed: 'bg-rc-accent-success',
    failed: 'bg-rc-accent-error',
    running: 'animate-pulse bg-rc-accent-warning',
  };

  return <span className={cn('h-2 w-2 rounded-full', styles[status])} />;
}

function SessionTaskRow({ task }: { task: SessionTaskItem }) {
  return (
    <div
      className="mt-1.5 flex items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors hover:bg-rc-bg-hover"
      style={{ paddingLeft: `${20 + task.depth * 16}px` }}
    >
      <StatusDot status={task.status} />
      <div className="min-w-0 flex-1">
        <div className="truncate font-medium text-rc-text-primary">{task.title}</div>
        {task.detail && task.detail !== i18n.t('chatArea.waitingSubagent') && (
          <div className="mt-0.5 truncate text-xs text-rc-text-tertiary">{task.detail}</div>
        )}
      </div>
    </div>
  );
}

function SessionRow({
  session,
  active,
  tasks,
  expanded,
  privacyMode,
  searchQuery,
  renaming,
  onToggleExpanded,
  onSelect,
  onArchive,
  onContextMenu,
  onRenameDone,
}: {
  session: SessionSummary;
  active: boolean;
  tasks: SessionTaskItem[];
  expanded: boolean;
  privacyMode: boolean;
  searchQuery: string;
  renaming: boolean;
  onToggleExpanded: () => void;
  onSelect: () => void;
  onArchive: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  onRenameDone: () => void;
}) {
  const { t } = useTranslation();
  const hasTasks = tasks.length > 0;
  const [renameVal, setRenameVal] = useState(session.title);

  return (
    <div className="space-y-0.5">
      <div
        className={cn(
          'group mx-2 flex items-start gap-2 rounded-lg border px-2.5 py-2.5 transition-colors duration-150',
          active
            ? 'border-transparent bg-rc-bg-selected shadow-xs'
            : 'border-transparent hover:bg-rc-bg-hover',
        )}
        onContextMenu={onContextMenu}
      >
        <button
          type="button"
          onClick={hasTasks ? onToggleExpanded : undefined}
          disabled={!hasTasks}
          title={hasTasks ? t('sidebar.expandTasks') : undefined}
          className={cn(
            'mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-md transition-colors',
            hasTasks
              ? 'text-rc-text-tertiary hover:bg-rc-bg-active hover:text-rc-text-primary'
              : 'cursor-default opacity-30',
          )}
        >
          <ChevronRight
            size={14}
            className={cn('transition-transform duration-200', expanded && 'rotate-90')}
          />
        </button>

        <button type="button" onClick={renaming ? undefined : onSelect} className="min-w-0 flex-1 text-left">
          {renaming ? (
            <input
              autoFocus
              value={renameVal}
              onChange={(e) => setRenameVal(e.target.value)}
              onBlur={() => {
                onRenameDone();
                if (renameVal.trim()) void useAppStore.getState().renameSession(session.id, renameVal.trim());
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  onRenameDone();
                  if (renameVal.trim()) void useAppStore.getState().renameSession(session.id, renameVal.trim());
                }
                if (e.key === 'Escape') onRenameDone();
              }}
              className="w-full rounded border border-rc-border-focus bg-rc-bg-tertiary px-1.5 py-0.5 text-sm text-rc-text-primary outline-none"
            />
          ) : (
            <>
              <div className="truncate text-sm font-medium text-rc-text-primary">
                {privacyMode ? t('sidebar.sessionHidden') : (
                  <HighlightedText text={session.title} query={searchQuery} />
                )}
              </div>
              <div className="mt-1 flex min-w-0 items-center gap-2 text-xs text-rc-text-tertiary">
                <span className="truncate">
                  {session.provider_name}
                  {session.model && <span className="mx-1">·</span>}
                  {session.model}
                </span>
                <span>·</span>
                <span>{formatRelativeTime(session.updated_at)}</span>
              </div>
            </>
          )}
        </button>

        <button
          type="button"
          onClick={onArchive}
          className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-rc-text-tertiary opacity-0 transition-all duration-200 group-hover:opacity-100 hover:bg-rc-bg-active hover:text-rc-accent-error"
          title={t('sidebar.archiveSession')}
        >
          <Archive size={14} />
        </button>
      </div>

      {/* CSS Grid collapse for task expansion */}
      <div className="grid-collapse pl-2" data-collapsed={!expanded}>
        <div className="grid-collapse-inner space-y-0.5">
          {tasks.map((task) => (
            <SessionTaskRow key={task.id} task={task} />
          ))}
        </div>
      </div>
    </div>
  );
}

// ── Time-bucketed session groups ──────────────────────────────

function SessionTimeGroups({
  sessions,
  activeSessionId,
  privacyMode,
  searchQuery,
  activeSessionTasks,
  liveSessionTasks,
  expandedSessions,
  renamingSessionId,
  onToggleSessionTasks,
  onSelectSession,
  onArchiveSession,
  setProjectPath,
  onSessionContextMenu,
  onRenameDone,
}: {
  sessions: SessionSummary[];
  activeSessionId: string | null;
  privacyMode: boolean;
  searchQuery: string;
  activeSessionTasks: SessionTaskItem[];
  liveSessionTasks: Record<string, SessionTaskItem[]>;
  expandedSessions: Record<string, boolean>;
  renamingSessionId: string | null;
  onToggleSessionTasks: (id: string) => void;
  onSelectSession: (session: SessionSummary) => void;
  onArchiveSession: (id: string) => void;
  setProjectPath: () => void;
  onSessionContextMenu?: (session: SessionSummary) => (e: React.MouseEvent) => void;
  onRenameDone: () => void;
}) {
  const bucketed = useMemo(() => {
    const map = new Map<TimeBucket, SessionSummary[]>();
    for (const bucket of BUCKET_ORDER) map.set(bucket, []);
    const sorted = [...sessions].sort(
      (a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
    );
    for (const session of sorted) {
      const bucket = getTimeBucket(session.updated_at);
      map.get(bucket)!.push(session);
    }
    return map;
  }, [sessions]);

  const hasMultipleBuckets = BUCKET_ORDER.filter((b) => (bucketed.get(b)?.length ?? 0) > 0).length > 1;

  if (sessions.length === 0) {
    return <div className="px-8 py-2 text-xs text-rc-text-tertiary">{i18n.t('sidebar.noSessions')}</div>;
  }

  return (
    <>
      {BUCKET_ORDER.map((bucket) => {
        const bucketSessions = bucketed.get(bucket);
        if (!bucketSessions?.length) return null;

        return (
          <div key={bucket}>
            {hasMultipleBuckets && (
              <div className="flex items-center gap-2 px-5 py-1.5 text-[10px] uppercase tracking-[0.08em] text-rc-text-tertiary">
                <Clock size={10} />
                {getBucketLabel(bucket)}
              </div>
            )}
            {bucketSessions.map((session) => (
              <SessionRow
                key={session.id}
                session={session}
                active={session.id === activeSessionId}
                privacyMode={privacyMode}
                searchQuery={searchQuery}
                tasks={session.id === activeSessionId ? activeSessionTasks : liveSessionTasks[session.id] ?? []}
                expanded={!!expandedSessions[session.id]}
                renaming={renamingSessionId === session.id}
                onToggleExpanded={() => onToggleSessionTasks(session.id)}
                onSelect={() => {
                  setProjectPath();
                  onSelectSession(session);
                }}
                onArchive={() => onArchiveSession(session.id)}
                onContextMenu={(e) => onSessionContextMenu?.(session)(e)}
                onRenameDone={onRenameDone}
              />
            ))}
          </div>
        );
      })}
    </>
  );
}

export function Sidebar() {
  const { t } = useTranslation();
  const sessions = useAppStore((state) => state.sessions);
  const sessionsLoading = useAppStore((state) => state.sessionsLoading);
  const sessionError = useAppStore((state) => state.sessionError);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const selectSession = useAppStore((state) => state.selectSession);
  const createSession = useAppStore((state) => state.createSession);
  const projects = useAppStore((state) => state.projects);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const setActiveProject = useAppStore((state) => state.setActiveProject);
  const removeProject = useAppStore((state) => state.removeProject);
  const archiveSession = useAppStore((state) => state.archiveSession);
  const renameSession = useAppStore((state) => state.renameSession);
  const togglePinSession = useAppStore((state) => state.togglePinSession);
  const toggleUnreadSession = useAppStore((state) => state.toggleUnreadSession);
  const pinnedSessions = useAppStore((state) => state.pinnedSessions);
  const unreadSessions = useAppStore((state) => state.unreadSessions);
  const pickFolderAndAddProject = useAppStore((state) => state.pickFolderAndAddProject);
  const openFileExplorer = useAppStore((state) => state.openFileExplorer);
  const conversation = useAppStore((state) => state.conversation);
  const sessionTasks = useAgentStore((state) => state.sessionTasks);

  // Context menu for sidebar sessions
  const { show: showMenu, MenuComponent } = useContextMenu();
  const [projectHoverKey, setProjectHoverKey] = useState<string | null>(null);

  /** Build context menu items for a sidebar session */
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
            navigator.clipboard.writeText(session.cwd).catch(() => {});
          },
        },
        {
          key: 'copy-path',
          label: t('contextMenu.copyPath'),
          icon: <Copy size={13} />,
          action: () => { navigator.clipboard.writeText(session.cwd).catch(() => {}); },
        },
        {
          key: 'copy-task-path',
          label: t('contextMenu.copyTaskPath'),
          icon: <Copy size={13} />,
          action: () => { navigator.clipboard.writeText(session.cwd).catch(() => {}); },
        },
        {
          key: 'copy-session-id',
          label: t('contextMenu.copySessionId'),
          icon: <FileText size={13} />,
          action: () => { navigator.clipboard.writeText(session.id).catch(() => {}); },
        },
        { key: 'sep2', label: '', separator: true, action: () => {} },
        {
          key: 'go-settings',
          label: t('contextMenu.goToSettings'),
          icon: <SlidersHorizontal size={13} />,
          action: () => {
            // Navigate to settings by emitting a custom event or setting state
            const event = new CustomEvent('navigate-to-settings');
            window.dispatchEvent(event);
          },
        },
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
          action: () => {},
        },
      ];
    },
    [t, pinnedSessions, unreadSessions, togglePinSession, toggleUnreadSession, archiveSession, renameSession, selectSession],
  );

  const [expandedProjects, setExpandedProjects] = useState<Record<string, boolean>>({});
  const [expandedSessions, setExpandedSessions] = useState<Record<string, boolean>>({});
  const [renamingSessionId, setRenamingSessionId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const debouncedSearch = useDebouncedValue(searchQuery, 150);

  const projectSessionGroups = useMemo(() => {
    const projectMap = new Map<string, SessionSummary[]>();
    for (const project of projects) {
      projectMap.set(normalizePathKey(project.path), []);
    }
    for (const session of sessions) {
      const key = normalizePathKey(session.cwd);
      projectMap.get(key)?.push(session);
    }
    return { projectMap };
  }, [projects, sessions]);

  const liveSessionTasks = useMemo(() => {
    const result: Record<string, SessionTaskItem[]> = {};
    Object.entries(sessionTasks).forEach(([sessionId, tasks]) => {
      result[sessionId] = flattenLiveTasks(tasks);
    });
    return result;
  }, [sessionTasks]);

  const activeSessionTasks = useMemo(() => {
    if (!activeSessionId) return [];
    const liveTasks = liveSessionTasks[activeSessionId] ?? [];
    if (liveTasks.length > 0) return liveTasks;
    return deriveAgentTasks(conversation);
  }, [activeSessionId, conversation.length, liveSessionTasks]);

  const normalizedSearch = debouncedSearch.trim().toLowerCase();
  const visibleProjectRows = useMemo(() => {
    return projects
      .map((project) => {
        const projectKey = normalizePathKey(project.path);
        const projectSessions = projectSessionGroups.projectMap.get(projectKey) ?? [];
        if (!normalizedSearch) return { project, sessions: projectSessions };
        const projectSearchText = privacyMode ? project.name : `${project.name} ${project.path}`;
        const projectMatches = projectSearchText.toLowerCase().includes(normalizedSearch);
        const matchedSessions = projectSessions.filter((session) =>
          [privacyMode ? '' : session.title, privacyMode ? '' : session.cwd, session.provider_name, session.model ?? '']
            .join(' ').toLowerCase().includes(normalizedSearch),
        );
        if (!projectMatches && matchedSessions.length === 0) return null;
        return { project, sessions: projectMatches ? projectSessions : matchedSessions };
      })
      .filter((row): row is { project: typeof projects[number]; sessions: SessionSummary[] } => !!row);
  }, [normalizedSearch, privacyMode, projectSessionGroups.projectMap, projects]);

  useEffect(() => {
    if (!activeProjectPath) return;
    const key = normalizePathKey(activeProjectPath);
    setExpandedProjects((state) => ({ ...state, [key]: true }));
  }, [activeProjectPath]);

  useEffect(() => {
    if (!activeSessionId || activeSessionTasks.length === 0) return;
    setExpandedSessions((state) => ({ ...state, [activeSessionId]: true }));
  }, [activeSessionId, activeSessionTasks.length]);

  const toggleProject = (path: string) => {
    const key = normalizePathKey(path);
    setExpandedProjects((state) => ({ ...state, [key]: !state[key] }));
    setActiveProject(normalizePathKey(activeProjectPath ?? '') === key ? null : path);
  };

  const toggleSessionTasks = (sessionId: string) => {
    setExpandedSessions((state) => ({ ...state, [sessionId]: !state[sessionId] }));
  };

  return (
    <aside className="flex w-sidebar shrink-0 flex-col border-r border-rc-border-secondary bg-rc-bg-sidebar select-none">
      {/* Search bar */}
      <div className="relative px-3 pb-2 pt-2">
        <Search size={14} className="pointer-events-none absolute left-5 top-1/2 -translate-y-1/2 text-rc-text-tertiary" />
        <input
          value={searchQuery}
          onChange={(event) => setSearchQuery(event.target.value)}
          aria-label={t('sidebar.searchAriaLabel')}
          placeholder={t('sidebar.searchPlaceholder')}
          className="h-9 w-full rounded-lg border border-transparent bg-rc-bg-tertiary pl-8 pr-7 text-xs text-rc-text-primary outline-none transition-colors placeholder:text-rc-text-tertiary focus:border-rc-border-focus focus-visible:outline-none"
        />
        {searchQuery && (
          <button
            type="button"
            title={t('app.clearSearch')}
            onClick={() => setSearchQuery('')}
            className="absolute right-3 top-1/2 flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
          >
            <X size={12} />
          </button>
        )}
      </div>

      {/* Toolbar */}
      <div className="grid gap-1.5 border-b border-rc-border-secondary px-2 pb-3">
        <button
          onClick={() => { void createSession(undefined, activeProjectPath ?? undefined); }}
          aria-label={t('sidebar.newSession')}
          className="inline-flex h-8 w-full items-center justify-center gap-2 rounded-lg border border-rc-border-primary bg-rc-bg-surface px-2 text-xs font-semibold text-rc-text-primary shadow-xs transition-colors hover:border-rc-border-hover hover:bg-rc-bg-hover"
        >
          <Plus size={14} />
          {t('sidebar.newSession')}
        </button>
        <div className="flex items-center gap-1">
        <button
          onClick={() => { void pickFolderAndAddProject(); }}
          aria-label={t('sidebar.addProject')}
          className="inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          <FolderPlus size={13} />
          {t('sidebar.addProject')}
        </button>
        <div className="flex-1" />
        <span className="text-[10px] uppercase tracking-[0.08em] text-rc-text-tertiary">{t('sidebar.projectCount', { count: projects.length })}</span>
        </div>
      </div>

      {/* Content with scroll fade mask */}
      <div className="scroll-fade flex-1 overflow-y-auto">
        {sessionError ? (
          <div className="px-4 py-4 text-center">
            <div className="rounded-lg border border-rc-accent-error-border bg-rc-accent-error-bg px-3 py-3 text-xs text-rc-accent-error">
              {sessionError}
            </div>
            <button
              type="button"
              onClick={() => { void useAppStore.getState().refreshSessions(); }}
              className="mt-2 text-xs font-medium text-rc-accent-primary hover:underline"
            >
              {t('common.retry')}
            </button>
          </div>
        ) : sessionsLoading ? (
          <div className="flex items-center gap-2 px-4 py-4 text-xs text-rc-text-secondary">
            <div className="h-3 w-3 animate-spin rounded-full border-2 border-rc-border-primary border-t-rc-accent-primary" />
            {t('sidebar.loadingSessions')}
          </div>
        ) : (
          <div className="py-2">
            {projects.length === 0 ? (
              <div className="px-4 py-6 text-center text-xs text-rc-text-tertiary">
                <div className="mb-2">{t('sidebar.noProjects')}</div>
              </div>
            ) : visibleProjectRows.length === 0 ? (
              <div className="px-4 py-4 text-center text-xs text-rc-text-tertiary">
                {t('sidebar.noMatch')}
              </div>
            ) : (
              visibleProjectRows.map(({ project, sessions: projectSessions }) => {
                const projectKey = normalizePathKey(project.path);
                const expanded = normalizedSearch
                  ? true
                  : expandedProjects[projectKey] ?? normalizePathKey(activeProjectPath ?? '') === projectKey;
                const active = normalizePathKey(activeProjectPath ?? '') === projectKey;

                return (
                  <div key={project.path} className="mb-1.5">
                    <div
                      className={cn(
                        'group mx-2 flex items-center gap-1 rounded-lg border px-2 py-2 text-xs transition-colors',
                        active
                          ? 'border-transparent bg-rc-bg-selected text-rc-text-primary'
                          : 'border-transparent text-rc-text-secondary hover:bg-rc-bg-hover',
                      )}
                    >
                      <button
                        type="button"
                        onClick={() => toggleProject(project.path)}
                        className="flex h-5 w-5 items-center justify-center rounded hover:bg-rc-bg-active"
                      >
                        <ChevronRight size={12} className={cn('transition-transform', expanded && 'rotate-90')} />
                      </button>
                      {expanded ? (
                        <FolderOpen size={14} className="shrink-0 text-rc-accent-primary" />
                      ) : (
                        <Folder size={14} className="shrink-0 text-rc-text-tertiary" />
                      )}
                      <div
                        className="relative flex-1 truncate font-semibold"
                        onMouseEnter={() => setProjectHoverKey(projectKey)}
                        onMouseLeave={() => setProjectHoverKey(null)}
                      >
                        <HighlightedText text={project.name} query={debouncedSearch} />
                        {projectHoverKey === projectKey && (
                          <div className="absolute left-0 top-full z-30 mt-1 rounded-lg border border-rc-border-primary bg-rc-bg-surface py-1 shadow-lg">
                            <button
                              type="button"
                              onClick={() => {
                                void removeProject(project.path);
                                setProjectHoverKey(null);
                              }}
                              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-accent-error"
                            >
                              <Trash2 size={13} />
                              {t('projectHover.remove')}
                            </button>
                            <button
                              type="button"
                              onClick={() => {
                                openFileExplorer(project.path, project.name);
                                setProjectHoverKey(null);
                              }}
                              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary"
                            >
                              <FolderOpen size={13} />
                              {t('projectHover.viewFiles')}
                            </button>
                            <button
                              type="button"
                              onClick={() => {
                                setActiveProject(project.path);
                                setExpandedProjects((state) => ({ ...state, [projectKey]: true }));
                                void createSession(undefined, project.path);
                                setProjectHoverKey(null);
                              }}
                              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary"
                            >
                              <Plus size={13} />
                              {t('projectHover.newTask')}
                            </button>
                          </div>
                        )}
                      </div>
                      <button
                        type="button"
                        onClick={() => {
                          setActiveProject(project.path);
                          setExpandedProjects((state) => ({ ...state, [projectKey]: true }));
                          void createSession(undefined, project.path);
                        }}
                        className="flex h-5 w-5 items-center justify-center rounded opacity-0 transition-opacity hover:bg-rc-bg-hover group-hover:opacity-100"
                        title={t('sidebar.newSession')}
                        aria-label={t('sidebar.newSession')}
                      >
                        <Plus size={12} />
                      </button>
                      <button
                        type="button"
                        onClick={() => { void removeProject(project.path); }}
                        disabled={project.session_count > 0}
                        className="flex h-5 w-5 items-center justify-center rounded opacity-0 transition-opacity hover:bg-rc-bg-hover hover:text-rc-accent-error disabled:cursor-not-allowed disabled:opacity-30 group-hover:opacity-100"
                        title={project.session_count > 0 ? t('sidebar.cannotRemoveProject') : t('sidebar.removeProject')}
                        aria-label={project.session_count > 0 ? t('sidebar.cannotRemoveProject') : t('sidebar.removeProject')}
                      >
                        <Trash2 size={12} />
                      </button>
                    </div>

                    {/* CSS Grid collapse for project sessions */}
                    <div className="grid-collapse" data-collapsed={!expanded}>
                      <div className="grid-collapse-inner">
                        <SessionTimeGroups
                          sessions={projectSessions}
                          activeSessionId={activeSessionId}
                          privacyMode={privacyMode}
                          searchQuery={debouncedSearch}
                          activeSessionTasks={activeSessionTasks}
                          liveSessionTasks={liveSessionTasks}
                          expandedSessions={expandedSessions}
                          renamingSessionId={renamingSessionId}
                          onToggleSessionTasks={toggleSessionTasks}
                          onSelectSession={(session) => void selectSession(session.id)}
                          onArchiveSession={(id) => void archiveSession(id)}
                          setProjectPath={() => setActiveProject(project.path)}
                          onSessionContextMenu={(session) => (e) => showMenu(e, buildSessionMenu(session))}
                          onRenameDone={() => setRenamingSessionId(null)}
                        />
                      </div>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        )}
      </div>
      {MenuComponent}
    </aside>
  );
}
