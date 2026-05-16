import {
  Archive,
  Bot,
  ChevronRight,
  Folder,
  FolderOpen,
  FolderPlus,
  Plus,
  Trash2,
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
  onToggleExpanded,
  onSelect,
  onArchive,
}: {
  session: SessionSummary;
  active: boolean;
  tasks: SessionTaskItem[];
  expanded: boolean;
  onToggleExpanded: () => void;
  onSelect: () => void;
  onArchive: () => void;
}) {
  const hasTasks = tasks.length > 0;

  return (
    <div className="space-y-1">
      <div
        className={cn(
          'group flex items-start gap-2 rounded-lg px-3 py-2.5 transition-all duration-200',
          active
            ? 'border border-[#9cc4ff] bg-[#eef6ff] shadow-sm dark:border-rc-border-focus dark:bg-rc-bg-selected'
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
          <div className="truncate text-sm font-medium text-rc-text-primary">{session.title}</div>
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
  const setActiveProject = useAppStore((state) => state.setActiveProject);
  const removeProject = useAppStore((state) => state.removeProject);
  const archiveSession = useAppStore((state) => state.archiveSession);
  const pickFolderAndAddProject = useAppStore((state) => state.pickFolderAndAddProject);
  const conversation = useAppStore((state) => state.conversation);
  const sessionTasks = useAgentStore((state) => state.sessionTasks);
  const canCreateSession = !!activeProjectPath;

  const [expandedProjects, setExpandedProjects] = useState<Record<string, boolean>>({});
  const [expandedSessions, setExpandedSessions] = useState<Record<string, boolean>>({});

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
    if (liveTasks.length > 0) {
      return liveTasks;
    }
    return deriveAgentTasks(conversation);
  }, [activeSessionId, conversation, liveSessionTasks]);

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
    <>
      <aside className="my-5 flex h-[calc(100%-40px)] w-[336px] shrink-0 flex-col overflow-hidden rounded-lg border border-white/80 bg-white/80 shadow-[0_20px_60px_rgba(15,23,42,0.12)] backdrop-blur-xl dark:border-rc-border-primary dark:bg-rc-bg-surface/90">
        {/* Header */}
        <div className="border-b border-rc-border-primary/80 px-5 py-4">
          <div className="flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-[linear-gradient(135deg,#2563eb_0%,#0891b2_100%)] text-white shadow-sm">
              <Bot size={17} />
            </div>
            <div>
              <div className="text-sm font-semibold text-rc-text-primary">Remote Code</div>
              <div className="text-xs text-rc-text-tertiary">工作台</div>
            </div>
          </div>

          <div className="mt-4 grid grid-cols-2 gap-2">
            <button
              onClick={() => {
                if (canCreateSession) {
                  void createSession(undefined, activeProjectPath ?? undefined);
                }
              }}
              disabled={!canCreateSession}
              title={canCreateSession ? '在当前选中的项目下新建会话' : '请先选择或添加项目文件夹'}
              className="inline-flex items-center justify-center gap-2 rounded-lg bg-[linear-gradient(135deg,#2563eb_0%,#0891b2_100%)] px-3 py-2.5 text-sm font-medium text-white shadow-sm transition-all duration-200 hover:shadow-md active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Plus size={15} />
              新会话
            </button>
            <button
              onClick={() => {
                void pickFolderAndAddProject();
              }}
              className="inline-flex items-center justify-center gap-2 rounded-lg border border-rc-border-primary bg-white/80 px-3 py-2.5 text-sm font-medium text-rc-text-primary transition-all duration-200 hover:border-rc-border-hover hover:bg-rc-bg-hover active:scale-[0.98] dark:bg-rc-bg-elevated"
            >
              <FolderPlus size={15} />
              添加项目
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-3 py-4">
          {sessionsLoading ? (
            <div className="flex items-center gap-3 px-3 py-4 text-sm text-rc-text-secondary">
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-rc-border-primary border-t-rc-accent-primary" />
              正在加载会话…
            </div>
          ) : (
            <div className="space-y-4">
              <section>
                <div className="mb-3 flex items-center justify-between px-2">
                  <div className="text-[11px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
                    项目
                  </div>
                  <button
                    onClick={() => {
                      void pickFolderAndAddProject();
                    }}
                    className="rounded-lg p-1 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
                    title="添加项目文件夹"
                  >
                    <FolderPlus size={13} />
                  </button>
                </div>

                {projects.length === 0 ? (
                  <div className="rounded-lg border border-dashed border-rc-border-primary px-4 py-6 text-center text-sm leading-6 text-rc-text-secondary">
                    还没有项目目录
                    <br />
                    <span className="text-xs text-rc-text-tertiary">添加项目后，会话将按项目管理</span>
                  </div>
                ) : (
                  <div className="space-y-3">
                    {projects.map((project) => {
                      const projectKey = normalizePathKey(project.path);
                      const projectSessions = projectSessionGroups.projectMap.get(projectKey) ?? [];
                      const expanded = expandedProjects[projectKey] ?? normalizePathKey(activeProjectPath ?? '') === projectKey;
                      const active = normalizePathKey(activeProjectPath ?? '') === projectKey;

                      return (
                        <section
                          key={project.path}
                          className={cn(
                            'rounded-lg px-3 py-3 transition-colors duration-200',
                            active
                              ? 'bg-white/90 border border-rc-border-primary shadow-sm dark:bg-rc-bg-elevated'
                              : 'bg-transparent border border-transparent hover:bg-rc-bg-hover',
                          )}
                        >
                          <div className="flex items-start gap-2">
                            <button
                              type="button"
                              onClick={() => toggleProject(project.path)}
                              title="展开/收起项目"
                              className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
                            >
                              <ChevronRight
                                size={15}
                                className={cn('transition-transform duration-200', expanded && 'rotate-90')}
                              />
                            </button>

                            <button
                              type="button"
                              onClick={() => {
                                setActiveProject(project.path);
                                setExpandedProjects((state) => ({ ...state, [projectKey]: true }));
                              }}
                              className="min-w-0 flex-1 text-left"
                            >
                              <div className="flex items-center gap-2">
                                {expanded ? (
                                  <FolderOpen size={16} className="text-rc-accent-primary" />
                                ) : (
                                  <Folder size={16} className="text-rc-text-secondary" />
                                )}
                                <span className="truncate text-sm font-semibold text-rc-text-primary">
                                  {project.name}
                                </span>
                              </div>
                              <div className="mt-1 truncate text-xs text-rc-text-tertiary">
                                {projectSessions.length} 个会话 · {truncateMiddle(project.path, 36)}
                              </div>
                            </button>

                            <button
                              type="button"
                              onClick={() => {
                                setActiveProject(project.path);
                                setExpandedProjects((state) => ({ ...state, [projectKey]: true }));
                                void createSession(undefined, project.path);
                              }}
                              className="rounded-md p-1.5 text-rc-text-tertiary transition-colors hover:bg-rc-accent-primary-light hover:text-rc-accent-primary"
                              title="在此项目下新建会话"
                            >
                              <Plus size={14} />
                            </button>

                            <button
                              type="button"
                              onClick={() => {
                                if (projectSessions.length === 0) {
                                  void removeProject(project.path);
                                }
                              }}
                              disabled={projectSessions.length > 0}
                              className="rounded-md p-1.5 text-rc-text-tertiary transition-colors hover:bg-rc-accent-error-bg hover:text-rc-accent-error disabled:cursor-not-allowed disabled:opacity-30 disabled:hover:bg-transparent"
                              title={
                                projectSessions.length > 0
                                  ? '该项目下仍有会话，无法移除'
                                  : '移除此项目'
                              }
                            >
                              <Trash2 size={14} />
                            </button>
                          </div>

                          {expanded && (
                            <div className="mt-3 space-y-1">
                              {projectSessions.length > 0 ? (
                                projectSessions.map((session) => (
                                  <SessionRow
                                    key={session.id}
                                    session={session}
                                    active={session.id === activeSessionId}
                                    tasks={
                                      session.id === activeSessionId
                                        ? activeSessionTasks
                                        : liveSessionTasks[session.id] ?? []
                                    }
                                    expanded={!!expandedSessions[session.id]}
                                    onToggleExpanded={() => toggleSessionTasks(session.id)}
                                    onSelect={() => {
                                      setActiveProject(project.path);
                                      void selectSession(session.id);
                                    }}
                                    onArchive={() => {
                                      void archiveSession(session.id);
                                    }}
                                  />
                                ))
                              ) : (
                                <div className="rounded-lg bg-rc-bg-hover px-3 py-3 text-xs text-rc-text-tertiary">
                                  这个项目下还没有会话
                                </div>
                              )}
                            </div>
                          )}
                        </section>
                      );
                    })}
                  </div>
                )}
              </section>
            </div>
          )}
        </div>
      </aside>
    </>
  );
}
