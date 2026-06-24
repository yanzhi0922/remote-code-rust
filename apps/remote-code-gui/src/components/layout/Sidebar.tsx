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
  Sparkles,
  Trash2,
  X,
  FolderPlus,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import type { ConversationEntry, SessionSubtask, SessionSummary, ToolCallInfo } from '../../lib/types';
import { cn, normalizePathKey, truncateMiddle, projectColor } from '../../lib/utils';
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
      className="mt-1 flex items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors hover:bg-rc-bg-hover"
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
  selected,
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
  selected?: boolean;
  onToggleExpanded: () => void;
  onSelect: (ctrlKey: boolean) => void;
  onArchive: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  onRenameDone: () => void;
}) {
  const { t } = useTranslation();
  const hasTasks = tasks.length > 0;
  const [renameVal, setRenameVal] = useState(session.title);

  return (
    <div className="space-y-1">
      <div
        className={cn(
          'group mx-2 flex items-start gap-2 rounded-md border px-2.5 py-2.5 transition-all duration-150',
          selected
            ? 'border-rc-accent-primary/25 bg-rc-bg-surface shadow-sm'
            : active
              ? 'border-rc-border-secondary bg-rc-bg-surface shadow-sm'
              : 'border-transparent hover:bg-rc-bg-hover hover:shadow-xs',
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

        <button type="button" onClick={renaming ? undefined : (e) => onSelect(e.ctrlKey || e.metaKey)} className="min-w-0 flex-1 text-left">
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
              className="w-full rounded-md border border-rc-border-focus bg-rc-bg-tertiary px-2 py-1 text-sm text-rc-text-primary outline-none"
            />
          ) : (
            <>
              <div className="flex min-w-0 items-center gap-1.5">
                {/* Agent type color dot — Codex-style category indicator */}
                <span
                  className="h-1.5 w-1.5 shrink-0 rounded-full"
                  style={{ backgroundColor: projectColor(session.agent_type) }}
                  aria-hidden="true"
                />
                <span className="truncate text-sm font-medium text-rc-text-primary">
                  {privacyMode ? t('sidebar.sessionHidden') : (
                    <HighlightedText text={session.title} query={searchQuery} />
                  )}
                </span>
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
  selectedSessionIds,
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
  onSelectSession: (session: SessionSummary, ctrlKey?: boolean) => void;
  onArchiveSession: (id: string) => void;
  setProjectPath: (session: SessionSummary) => void;
  onSessionContextMenu?: (session: SessionSummary) => (e: React.MouseEvent) => void;
  selectedSessionIds?: Set<string>;
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
                selected={selectedSessionIds?.has(session.id)}
                onToggleExpanded={() => onToggleSessionTasks(session.id)}
                onSelect={(ctrlKey) => {
                  setProjectPath(session);
                  onSelectSession(session, ctrlKey);
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
  const archivedSessions = useAppStore((state) => state.archivedSessions);
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
          key: 'copy-name',
          label: t('contextMenu.copyName'),
          icon: <Copy size={13} />,
          action: () => { navigator.clipboard.writeText(session.title).catch(() => {}); },
        },
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
          key: 'share-link',
          label: t('contextMenu.shareLink'),
          icon: <ExternalLink size={13} />,
          action: () => {
            const url = `${window.location.origin}/sessions/${session.id}`;
            navigator.clipboard.writeText(url).catch(() => {});
          },
        },
        {
          key: 'focus-mode',
          label: t('contextMenu.focusMode'),
          icon: <Sparkles size={13} />,
          action: () => {
            // Focus mode: collapse sidebar and ensure this session is
            // selected. We use toggleSidebar to mirror the keyboard shortcut.
            selectSession(session.id);
            window.dispatchEvent(new CustomEvent('workbench-focus-mode', { detail: { sessionId: session.id } }));
          },
        },
        { key: 'sep3', label: '', separator: true, action: () => {} },
        {
          key: 'go-settings',
          label: t('contextMenu.goToSettings'),
          icon: <SlidersHorizontal size={13} />,
          action: () => {
            window.dispatchEvent(new CustomEvent('navigate-to-settings'));
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
          action: () => {
            window.open('https://github.com/yanzhi0922/remote-code/issues', '_blank')?.focus();
          },
        },
      ];
    },
    [t, pinnedSessions, unreadSessions, togglePinSession, toggleUnreadSession, archiveSession, renameSession, selectSession],
  );

  const [expandedProjects, setExpandedProjects] = useState<Record<string, boolean>>({});
  const [expandedSessions, setExpandedSessions] = useState<Record<string, boolean>>({});
  const [renamingSessionId, setRenamingSessionId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedSessionIds, setSelectedSessionIds] = useState<Set<string>>(new Set());
  const [projectMenuOpen, setProjectMenuOpen] = useState(false);
  const [sessionFilter, setSessionFilter] = useState<'all' | 'pinned' | 'unread'>('all');
  const debouncedSearch = useDebouncedValue(searchQuery, 150);

  // Keyboard: Ctrl/Cmd+Shift+P opens the project quick switcher.
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'P' || e.key === 'p')) {
        e.preventDefault();
        setProjectMenuOpen((open) => !open);
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, []);

  // Multi-select: Ctrl+A and Escape
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'a' && selectedSessionIds.size > 0) {
        e.preventDefault();
        setSelectedSessionIds(new Set(sessions.map((s) => s.id)));
      }
      if (e.key === 'Escape' && selectedSessionIds.size > 0) {
        setSelectedSessionIds(new Set());
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [sessions, selectedSessionIds.size]);

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

  const visibleSessions = useMemo(() => {
    let base: SessionSummary[];
    if (normalizedSearch) {
      const seen = new Set<string>();
      base = visibleProjectRows.flatMap((row) =>
        row.sessions.filter((session) => {
          if (seen.has(session.id)) return false;
          seen.add(session.id);
          return true;
        }),
      );
    } else if (!activeProjectPath) {
      base = sessions;
    } else {
      const activeKey = normalizePathKey(activeProjectPath);
      base = sessions.filter((session) => normalizePathKey(session.cwd) === activeKey);
    }
    // Apply sticky filters (Pinned / Unread) on top of the project-scoped
    // base list so the user can quickly focus on what matters.
    if (sessionFilter === 'pinned') {
      base = base.filter((s) => pinnedSessions.has(s.id));
    } else if (sessionFilter === 'unread') {
      base = base.filter((s) => unreadSessions.has(s.id));
    }
    return base;
  }, [activeProjectPath, normalizedSearch, sessions, visibleProjectRows, sessionFilter, pinnedSessions, unreadSessions]);

  const activeProject = useMemo(() => {
    if (!projects.length) return null;
    const activeKey = normalizePathKey(activeProjectPath ?? '');
    return projects.find((project) => normalizePathKey(project.path) === activeKey) ?? projects[0];
  }, [activeProjectPath, projects]);

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
    <aside className="relative flex w-sidebar shrink-0 flex-col border-r border-rc-border-primary bg-white/28 pt-[74px] select-none">
      <div className="px-4 pb-3">
        <button
          onClick={() => { void createSession(undefined, activeProjectPath ?? undefined); }}
          aria-label={t('sidebar.newSession')}
          className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-full border border-rc-border-primary bg-rc-bg-elevated px-3 text-sm font-semibold text-rc-text-primary shadow-sm transition-all hover:-translate-y-0.5 hover:bg-white hover:shadow-md"
        >
          <Plus size={15} />
          {t('sidebar.newSession')}
        </button>
      </div>

      {/* ── Sidebar top-right: refresh + reveal shortcuts (Codex-style) ── */}
      <div className="px-4 pb-2 flex items-center gap-1.5">
        <button
          type="button"
          onClick={() => { void useAppStore.getState().refreshSessions(); }}
          aria-label={t('sidebar.refreshSessions')}
          title={t('sidebar.refreshSessions')}
          className="inline-flex h-7 items-center gap-1 rounded-full border border-rc-border-secondary bg-rc-bg-elevated/40 px-2.5 text-[11px] text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          <RefreshCw size={11} />
          {t('common.retry').replace(/^Retry$/, t('sidebar.refreshSessions').slice(0, 6))}
        </button>
        {activeProject && (
          <button
            type="button"
            onClick={() => void openFileExplorer(activeProject.path, activeProject.name)}
            aria-label={t('sidebar.openInExplorer')}
            title={t('sidebar.openInExplorer')}
            className="inline-flex h-7 items-center gap-1 rounded-full border border-rc-border-secondary bg-rc-bg-elevated/40 px-2.5 text-[11px] text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
          >
            <ExternalLink size={11} />
          </button>
        )}
        <div className="flex-1" />
        {archivedSessions.length > 0 && (
          <span
            className="rounded-full bg-rc-accent-warning-bg px-2 py-0.5 text-[10px] font-medium text-rc-accent-warning"
            title={t('sidebar.batchArchive')}
          >
            {archivedSessions.length}
          </span>
        )}
      </div>

      <div className="relative px-4 pb-3">
        <Search size={14} className="pointer-events-none absolute left-7 top-1/2 -translate-y-1/2 text-rc-text-tertiary" />
        <input
          value={searchQuery}
          onChange={(event) => setSearchQuery(event.target.value)}
          aria-label={t('sidebar.searchAriaLabel')}
          placeholder={t('sidebar.searchPlaceholder')}
          className="h-10 w-full rounded-full border border-rc-border-primary bg-rc-bg-surface pl-8 pr-8 text-xs text-rc-text-primary shadow-xs outline-none transition-colors placeholder:text-rc-text-tertiary focus:border-rc-border-focus focus-visible:outline-none"
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

      <div className="px-4 pb-4">
        <div className="mb-2 flex items-center justify-between">
          <span className="text-[10px] font-semibold uppercase tracking-[0.16em] text-rc-text-tertiary">
            {t('sidebar.sessions')}
          </span>
          <span className="text-[10px] text-rc-text-tertiary">
            {visibleSessions.length}
          </span>
        </div>
        <div className="relative flex items-center gap-2">
          <button
            type="button"
            onClick={() => setProjectMenuOpen((open) => !open)}
            className="inline-flex h-8 min-w-0 flex-1 items-center gap-2 rounded-full border border-rc-border-primary bg-rc-bg-surface px-3 text-left text-[11px] text-rc-text-secondary shadow-xs transition-all hover:bg-rc-bg-hover hover:text-rc-text-primary"
            title={activeProject ? (privacyMode ? activeProject.name : activeProject.path) : t('sidebar.noProjects')}
          >
            {activeProject ? (
              <>
                <span className="h-1.5 w-1.5 shrink-0 rounded-full" style={{ backgroundColor: projectColor(activeProject.name) }} />
                <span className="min-w-0 flex-1 truncate">{activeProject.name}</span>
                <span className="shrink-0 text-rc-text-tertiary">{t('sidebar.projectCount', { count: projects.length })}</span>
              </>
            ) : (
              <span className="min-w-0 flex-1 truncate">{t('sidebar.noProjects')}</span>
            )}
            <ChevronRight size={12} className={cn('shrink-0 transition-transform', projectMenuOpen && 'rotate-90')} />
          </button>
          <button
            onClick={() => { void pickFolderAndAddProject(); }}
            aria-label={t('sidebar.addProject')}
            className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-dashed border-rc-border-secondary text-rc-text-secondary transition-colors hover:bg-rc-bg-surface hover:text-rc-text-primary"
          >
            <FolderPlus size={13} />
          </button>
          {projectMenuOpen && (
            <>
              <button
                type="button"
                aria-label={t('chatInput.closeDropdown')}
                className="fixed inset-0 z-10 cursor-default"
                onClick={() => setProjectMenuOpen(false)}
              />
              <div className="codex-popover absolute left-0 right-10 top-full z-20 mt-2 p-1.5 animate-fade-in-up">
                {projects.map((project) => {
                  const active = normalizePathKey(activeProjectPath ?? '') === normalizePathKey(project.path);
                  return (
                    <div key={project.path} className="group/project flex items-center gap-1 rounded-md hover:bg-rc-bg-hover">
                      <button
                        type="button"
                        onClick={() => {
                          setActiveProject(active ? null : project.path);
                          setProjectMenuOpen(false);
                        }}
                        className={cn(
                          'flex min-w-0 flex-1 items-center gap-2 rounded-md px-2.5 py-2 text-left text-xs',
                          active ? 'text-rc-text-primary' : 'text-rc-text-secondary',
                        )}
                      >
                        <span className="h-1.5 w-1.5 shrink-0 rounded-full" style={{ backgroundColor: projectColor(project.name) }} />
                        <span className="min-w-0 flex-1 truncate">{project.name}</span>
                        <span className="text-[10px] text-rc-text-tertiary">{project.session_count}</span>
                      </button>
                      <button
                        type="button"
                        onClick={() => { void removeProject(project.path); }}
                        disabled={project.session_count > 0}
                        className="mr-1 flex h-6 w-6 items-center justify-center rounded-full text-rc-text-tertiary opacity-0 transition-all hover:bg-rc-bg-hover hover:text-rc-accent-error disabled:cursor-not-allowed disabled:hover:bg-transparent disabled:hover:text-rc-text-tertiary group-hover/project:opacity-100"
                        title={project.session_count > 0 ? t('sidebar.cannotRemoveProject') : t('sidebar.removeProject')}
                        aria-label={project.session_count > 0 ? t('sidebar.cannotRemoveProject') : t('sidebar.removeProject')}
                      >
                        <X size={12} />
                      </button>
                    </div>
                  );
                })}
              </div>
            </>
          )}
        </div>
      </div>

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
          <div className="pb-4">
            {projects.length === 0 && !normalizedSearch ? (
              <div className="mx-4 mt-2 rounded-md border border-dashed border-rc-border-primary bg-rc-bg-elevated/30 px-4 py-6 text-center">
                <Folder size={24} className="mx-auto mb-2 text-rc-text-tertiary opacity-60" />
                <div className="mb-1 text-xs font-medium text-rc-text-secondary">
                  {t('sidebar.noProjects')}
                </div>
                <p className="mb-3 text-[10px] leading-4 text-rc-text-tertiary">
                  {t('sidebar.addProjectHint')}
                </p>
                <div className="flex items-center justify-center gap-2">
                  <button
                    type="button"
                    onClick={() => { void pickFolderAndAddProject(); }}
                    className="inline-flex items-center gap-1 rounded-full bg-rc-accent-primary px-3 py-1 text-[11px] font-medium text-white shadow-xs hover:bg-rc-accent-primary-hover"
                  >
                    <FolderPlus size={11} />
                    {t('sidebar.addProject')}
                  </button>
                  <button
                    type="button"
                    onClick={() => { void openFileExplorer('', ''); }}
                    className="inline-flex items-center gap-1 rounded-full border border-rc-border-primary px-3 py-1 text-[11px] font-medium text-rc-text-secondary hover:bg-rc-bg-hover"
                  >
                    <ExternalLink size={11} />
                    {t('sidebar.openInExplorer')}
                  </button>
                </div>
              </div>
            ) : visibleSessions.length === 0 ? (
              <div className="px-4 py-4 text-center text-xs text-rc-text-tertiary">
                {t('sidebar.noMatch')}
              </div>
            ) : (
              <>
              {/* Filter chips: All / Pinned / Unread — Codex-style sticky filters */}
              <div className="px-4 pb-2 flex items-center gap-1.5" role="tablist" aria-label={t('sidebar.filterSessions')}>
                {(['all', 'pinned', 'unread'] as const).map((kind) => {
                  const active = sessionFilter === kind;
                  const count = kind === 'pinned' ? pinnedSessions.size
                    : kind === 'unread' ? unreadSessions.size
                    : sessions.length;
                  return (
                    <button
                      key={kind}
                      type="button"
                      role="tab"
                      aria-selected={active}
                      data-testid={`sidebar-filter-${kind}`}
                      onClick={() => setSessionFilter(kind)}
                      className={`inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-[11px] font-medium transition-colors ${
                        active
                          ? 'bg-rc-bg-active text-rc-text-primary shadow-xs'
                          : 'text-rc-text-secondary hover:bg-rc-bg-hover'
                      }`}
                    >
                      <span>{t(`sidebar.filter.${kind}`)}</span>
                      <span className={`rounded-full px-1 text-[10px] ${
                        active ? 'bg-rc-bg-surface text-rc-text-tertiary' : 'text-rc-text-tertiary'
                      }`}>
                        {count}
                      </span>
                    </button>
                  );
                })}
              </div>
              <SessionTimeGroups
                sessions={visibleSessions}
                activeSessionId={activeSessionId}
                privacyMode={privacyMode}
                searchQuery={debouncedSearch}
                activeSessionTasks={activeSessionTasks}
                liveSessionTasks={liveSessionTasks}
                expandedSessions={expandedSessions}
                renamingSessionId={renamingSessionId}
                onToggleSessionTasks={toggleSessionTasks}
                onSelectSession={(session, ctrlKey) => {
                  if (ctrlKey) {
                    setSelectedSessionIds((prev) => {
                      const next = new Set(prev);
                      if (next.has(session.id)) next.delete(session.id);
                      else next.add(session.id);
                      return next;
                    });
                  } else {
                    if (selectedSessionIds.size > 0) setSelectedSessionIds(new Set());
                    void selectSession(session.id);
                  }
                }}
                onArchiveSession={(id) => void archiveSession(id)}
                setProjectPath={(session) => setActiveProject(session.cwd)}
                onSessionContextMenu={(session) => (e) => showMenu(e, buildSessionMenu(session))}
                selectedSessionIds={selectedSessionIds}
                onRenameDone={() => setRenamingSessionId(null)}
              />
              </>
            )}
          </div>
        )}
      </div>
      {MenuComponent}

      {selectedSessionIds.size > 0 && (
        <div className="absolute bottom-3 left-3 right-3 z-20 flex items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2 shadow-lg animate-fade-in-up">
          <span className="text-xs font-medium text-rc-text-secondary">{t('sidebar.selectedCount', { count: selectedSessionIds.size })}</span>
          <div className="flex-1" />
          <button
            type="button"
            onClick={() => {
              selectedSessionIds.forEach((id) => void archiveSession(id));
              setSelectedSessionIds(new Set());
            }}
            className="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-accent-error"
          >
            <Archive size={12} />
            {t('sidebar.batchArchive')}
          </button>
          <button
            type="button"
            onClick={() => {
              selectedSessionIds.forEach((id) => togglePinSession(id));
              setSelectedSessionIds(new Set());
            }}
            className="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary"
          >
            <Pin size={12} />
            {t('sidebar.batchPin')}
          </button>
          <button
            type="button"
            onClick={() => setSelectedSessionIds(new Set())}
            className="flex items-center justify-center rounded-md p-1 text-rc-text-tertiary hover:bg-rc-bg-hover hover:text-rc-text-primary"
          >
            <X size={12} />
          </button>
        </div>
      )}
    </aside>
  );
}
