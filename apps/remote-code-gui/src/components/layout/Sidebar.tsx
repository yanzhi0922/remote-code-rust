import {
  Archive,
  ChevronRight,
  Folder,
  FolderOpen,
  FolderPlus,
  Plus,
  Search,
  Trash2,
  X,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { ConversationEntry, SessionSubtask, SessionSummary, ToolCallInfo } from '../../lib/types';
import { cn, normalizePathKey, truncateMiddle } from '../../lib/utils';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';

type SessionTaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'stopped';

interface SessionTaskItem {
  id: string;
  title: string;
  status: SessionTaskStatus;
  detail: string;
  depth: number;
}

function formatRelativeTime(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diffMinutes = Math.floor((now - then) / 60_000);
  if (diffMinutes < 1) return '刚刚';
  if (diffMinutes < 60) return `${diffMinutes} 分钟前`;
  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) return `${diffHours} 小时前`;
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 7) return `${diffDays} 天前`;
  return new Date(iso).toLocaleDateString('zh-CN');
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
  return '子任务';
}

function summarizeAgentOutput(text: string): string {
  const parsed = parseJson(text);
  if (parsed?.type === 'batch_delegation_result') {
    const total = typeof parsed.total === 'number' ? parsed.total : null;
    const succeeded = typeof parsed.succeeded === 'number' ? parsed.succeeded : null;
    if (total !== null && succeeded !== null) {
      return `批量完成 ${succeeded}/${total}`;
    }
  }
  const compact = text.replace(/\s+/g, ' ').trim();
  return compact ? truncateMiddle(compact, 56) : '已完成';
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
            detail: '等待子代理结果',
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
        detail: '等待子代理结果',
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
      detail: truncateMiddle(task.output_preview ?? task.summary ?? '等待子代理结果', 56),
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
        {task.detail && task.detail !== '等待子代理结果' && (
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
  onToggleExpanded,
  onSelect,
  onArchive,
}: {
  session: SessionSummary;
  active: boolean;
  tasks: SessionTaskItem[];
  expanded: boolean;
  privacyMode: boolean;
  onToggleExpanded: () => void;
  onSelect: () => void;
  onArchive: () => void;
}) {
  const hasTasks = tasks.length > 0;

  return (
    <div className="space-y-1">
      <div
        className={cn(
          'group flex items-start gap-2 rounded-md px-2.5 py-2 transition-colors duration-150',
          active
            ? 'border border-rc-border-focus bg-rc-bg-selected'
            : 'border border-transparent hover:bg-rc-bg-hover',
        )}
      >
        <button
          type="button"
          onClick={hasTasks ? onToggleExpanded : undefined}
          disabled={!hasTasks}
          title={hasTasks ? '展开/收起子任务' : undefined}
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

        <button type="button" onClick={onSelect} className="min-w-0 flex-1 text-left">
          <div className="truncate text-sm font-medium text-rc-text-primary">
            {privacyMode ? '会话已隐藏' : session.title}
          </div>
          <div className="mt-1 flex items-center gap-2 text-xs text-rc-text-tertiary">
            <span className="truncate">
              {session.provider_name}
              {session.model && <span className="mx-1">·</span>}
              {session.model}
            </span>
            <span>·</span>
            <span>{formatRelativeTime(session.updated_at)}</span>
          </div>
        </button>

        <button
          type="button"
          onClick={onArchive}
          className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-rc-text-tertiary opacity-0 transition-all duration-200 group-hover:opacity-100 hover:bg-rc-bg-active hover:text-rc-accent-error"
          title="归档此会话"
        >
          <Archive size={14} />
        </button>
      </div>

      {expanded && hasTasks && (
        <div className="space-y-0.5 pl-2">
          {tasks.map((task) => (
            <SessionTaskRow key={task.id} task={task} />
          ))}
        </div>
      )}
    </div>
  );
}

export function Sidebar() {
  const sessions = useAppStore((state) => state.sessions);
  const sessionsLoading = useAppStore((state) => state.sessionsLoading);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const selectSession = useAppStore((state) => state.selectSession);
  const createSession = useAppStore((state) => state.createSession);
  const projects = useAppStore((state) => state.projects);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const setActiveProject = useAppStore((state) => state.setActiveProject);
  const removeProject = useAppStore((state) => state.removeProject);
  const archiveSession = useAppStore((state) => state.archiveSession);
  const pickFolderAndAddProject = useAppStore((state) => state.pickFolderAndAddProject);
  const conversation = useAppStore((state) => state.conversation);
  const sessionTasks = useAgentStore((state) => state.sessionTasks);

  const [expandedProjects, setExpandedProjects] = useState<Record<string, boolean>>({});
  const [expandedSessions, setExpandedSessions] = useState<Record<string, boolean>>({});
  const [searchQuery, setSearchQuery] = useState('');

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
  }, [activeSessionId, conversation, liveSessionTasks]);

  const normalizedSearch = searchQuery.trim().toLowerCase();
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
      <div className="relative border-b border-rc-border-secondary px-3 py-2">
        <Search size={14} className="pointer-events-none absolute left-5 top-1/2 -translate-y-1/2 text-rc-text-tertiary" />
        <input
          value={searchQuery}
          onChange={(event) => setSearchQuery(event.target.value)}
          aria-label="搜索项目和会话"
          placeholder="搜索项目或会话"
          className="h-7 w-full rounded border border-transparent bg-rc-bg-tertiary pl-7 pr-7 text-xs text-rc-text-primary outline-none transition-colors placeholder:text-rc-text-tertiary focus:border-rc-border-focus"
        />
        {searchQuery && (
          <button
            type="button"
            title="清空搜索"
            onClick={() => setSearchQuery('')}
            className="absolute right-3 top-1/2 flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
          >
            <X size={12} />
          </button>
        )}
      </div>

      {/* Toolbar */}
      <div className="flex items-center gap-1 border-b border-rc-border-secondary px-2 py-1.5">
        <button
          onClick={() => { void pickFolderAndAddProject(); }}
          className="inline-flex items-center gap-1 rounded px-2 py-1 text-xs text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          <FolderPlus size={13} />
          Add Project
        </button>
        <div className="flex-1" />
        <span className="text-[10px] uppercase tracking-[0.08em] text-rc-text-tertiary">{projects.length} projects</span>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {sessionsLoading ? (
          <div className="flex items-center gap-2 px-4 py-4 text-xs text-rc-text-secondary">
            <div className="h-3 w-3 animate-spin rounded-full border-2 border-rc-border-primary border-t-rc-accent-primary" />
            Loading…
          </div>
        ) : (
          <div className="py-2">
            {projects.length === 0 ? (
              <div className="px-4 py-6 text-center text-xs text-rc-text-tertiary">
                <div className="mb-2">No projects yet</div>
              </div>
            ) : visibleProjectRows.length === 0 ? (
              <div className="px-4 py-4 text-center text-xs text-rc-text-tertiary">
                No matching results
              </div>
            ) : (
              visibleProjectRows.map(({ project, sessions: projectSessions }) => {
                const projectKey = normalizePathKey(project.path);
                const expanded = normalizedSearch
                  ? true
                  : expandedProjects[projectKey] ?? normalizePathKey(activeProjectPath ?? '') === projectKey;
                const active = normalizePathKey(activeProjectPath ?? '') === projectKey;

                return (
                  <div key={project.path}>
                    <div
                      className={cn(
                        'group flex items-center gap-1 px-3 py-1.5 text-xs transition-colors',
                        active ? 'bg-rc-bg-active text-rc-text-primary' : 'text-rc-text-secondary hover:bg-rc-bg-hover',
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
                      <span className="flex-1 truncate font-medium">{project.name}</span>
                      <button
                        type="button"
                        onClick={() => {
                          setActiveProject(project.path);
                          setExpandedProjects((state) => ({ ...state, [projectKey]: true }));
                          void createSession(undefined, project.path);
                        }}
                        className="flex h-5 w-5 items-center justify-center rounded opacity-0 transition-opacity hover:bg-rc-bg-hover group-hover:opacity-100"
                        title="新会话"
                        aria-label="新会话"
                      >
                        <Plus size={12} />
                      </button>
                      <button
                        type="button"
                        onClick={() => { void removeProject(project.path); }}
                        disabled={project.session_count > 0}
                        className="flex h-5 w-5 items-center justify-center rounded opacity-0 transition-opacity hover:bg-rc-bg-hover hover:text-rc-accent-error disabled:cursor-not-allowed disabled:opacity-30 group-hover:opacity-100"
                        title={project.session_count > 0 ? '该项目下仍有会话，无法移除' : '移除此项目'}
                        aria-label={project.session_count > 0 ? '该项目下仍有会话，无法移除' : '移除此项目'}
                      >
                        <Trash2 size={12} />
                      </button>
                    </div>

                    {expanded && (
                      <div>
                        {projectSessions.length > 0 ? (
                          projectSessions.map((session) => (
                            <SessionRow
                              key={session.id}
                              session={session}
                              active={session.id === activeSessionId}
                              privacyMode={privacyMode}
                              tasks={session.id === activeSessionId ? activeSessionTasks : liveSessionTasks[session.id] ?? []}
                              expanded={!!expandedSessions[session.id]}
                              onToggleExpanded={() => toggleSessionTasks(session.id)}
                              onSelect={() => {
                                setActiveProject(project.path);
                                void selectSession(session.id);
                              }}
                              onArchive={() => { void archiveSession(session.id); }}
                            />
                          ))
                        ) : (
                          <div className="px-8 py-2 text-xs text-rc-text-tertiary">No sessions</div>
                        )}
                      </div>
                    )}
                  </div>
                );
              })
            )}
          </div>
        )}
      </div>
    </aside>
  );
}
