import {
  Archive,
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

function SessionTaskRow({ task }: { task: SessionTaskItem }) {
  return (
    <div
      className="mt-1 flex items-start gap-2 rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-3 py-2"
      style={{ marginLeft: `${28 + task.depth * 18}px` }}
    >
      <span
        className={cn(
          'mt-1 h-2.5 w-2.5 shrink-0 rounded-full',
          task.status === 'pending' && 'bg-rc-text-tertiary',
          task.status === 'stopped' && 'bg-rc-text-tertiary',
          task.status === 'completed' && 'bg-rc-accent-success',
          task.status === 'failed' && 'bg-rc-accent-error',
          task.status === 'running' && 'animate-pulse bg-rc-accent-warning',
        )}
      />
      <div className="min-w-0">
        <div className="truncate text-xs font-semibold text-rc-text-primary">{task.title}</div>
        <div className="mt-1 truncate text-[11px] text-rc-text-tertiary">{task.detail}</div>
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
          'flex items-start gap-2 rounded-[20px] border px-2.5 py-2 transition-colors',
          active ? 'border-rc-border-primary bg-rc-bg-active' : 'border-transparent bg-rc-bg-primary/70 hover:bg-rc-bg-primary',
        )}
      >
        <button
          type="button"
          onClick={hasTasks ? onToggleExpanded : undefined}
          disabled={!hasTasks}
          title={hasTasks ? '展开/收起子任务' : undefined}
          className={cn(
            'mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-rc-text-tertiary',
            hasTasks ? 'hover:bg-rc-bg-primary hover:text-rc-text-primary' : 'cursor-default opacity-30',
          )}
        >
          <ChevronRight size={14} className={expanded ? 'rotate-90 transition-transform' : 'transition-transform'} />
        </button>

        <button type="button" onClick={onSelect} className="min-w-0 flex-1 text-left">
          <div className="truncate text-sm font-medium text-rc-text-primary">{session.title}</div>
          <div className="mt-1 flex flex-wrap items-center gap-2 text-[11px] text-rc-text-tertiary">
            <span className="truncate">
              {session.provider_name}
              {session.model ? ` · ${session.model}` : ''}
            </span>
            <span>·</span>
            <span>{formatRelativeTime(session.updated_at)}</span>
          </div>
        </button>

        <button
          type="button"
          onClick={onArchive}
          className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-rc-text-tertiary transition-colors hover:bg-rc-bg-primary hover:text-rc-text-primary"
          title="归档此会话"
        >
          <Archive size={14} />
        </button>
      </div>

      {expanded && hasTasks && (
        <div className="space-y-1">
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
      <aside className="flex h-full w-[360px] shrink-0 flex-col border-r border-rc-border-primary bg-rc-bg-sidebar">
        <div className="border-b border-rc-border-primary px-4 py-4">
          <div className="text-xs font-semibold uppercase tracking-[0.24em] text-rc-text-tertiary">
            Workspace
          </div>
          <div className="mt-1 text-xl font-semibold text-rc-text-primary">Remote Code</div>
          <div className="mt-2 text-sm leading-6 text-rc-text-secondary">
            项目、会话、子任务按树形层级组织。
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
              className="inline-flex items-center justify-center gap-2 rounded-2xl bg-rc-bg-user-bubble px-3 py-2.5 text-sm font-medium text-rc-text-inverse transition-colors hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Plus size={15} />
              新会话
            </button>
            <button
              onClick={() => {
                void pickFolderAndAddProject();
              }}
              className="inline-flex items-center justify-center gap-2 rounded-2xl border border-rc-border-primary bg-rc-bg-primary px-3 py-2.5 text-sm font-medium text-rc-text-primary transition-colors hover:bg-rc-bg-hover"
            >
              <FolderPlus size={15} />
              添加项目
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto px-3 py-4">
          {sessionsLoading ? (
            <div className="px-3 py-4 text-sm text-rc-text-secondary">正在加载会话…</div>
          ) : (
            <div className="space-y-5">
              <section>
                <div className="mb-3 flex items-center justify-between px-2">
                  <div className="text-xs font-semibold uppercase tracking-[0.2em] text-rc-text-tertiary">
                    Managed Projects
                  </div>
                  <button
                    onClick={() => {
                      void pickFolderAndAddProject();
                    }}
                    className="rounded-full p-1 text-rc-text-tertiary transition-colors hover:bg-rc-bg-primary hover:text-rc-text-primary"
                    title="添加项目文件夹"
                  >
                    <FolderPlus size={14} />
                  </button>
                </div>

                {projects.length === 0 ? (
                  <div className="rounded-2xl border border-dashed border-rc-border-primary px-4 py-6 text-center text-sm leading-6 text-rc-text-secondary">
                    还没有手动管理的项目目录。
                    <br />
                    添加项目后，所有会话都会挂在项目节点下。
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
                            'rounded-[24px] border px-3 py-3 shadow-xs',
                            active ? 'border-rc-border-primary bg-rc-bg-primary' : 'border-transparent bg-rc-bg-secondary',
                          )}
                        >
                          <div className="flex items-start gap-2">
                            <button
                              type="button"
                              onClick={() => toggleProject(project.path)}
                              title="展开/收起项目"
                              className="mt-1 flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
                            >
                              <ChevronRight
                                size={15}
                                className={expanded ? 'rotate-90 transition-transform' : 'transition-transform'}
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
                                  <FolderOpen size={16} className="text-rc-text-secondary" />
                                ) : (
                                  <Folder size={16} className="text-rc-text-secondary" />
                                )}
                                <div className="truncate text-sm font-semibold text-rc-text-primary">{project.name}</div>
                              </div>
                              <div className="mt-1 truncate text-xs text-rc-text-secondary">
                                {projectSessions.length} 个会话 · {truncateMiddle(project.path, 44)}
                              </div>
                            </button>

                            <button
                              type="button"
                              onClick={() => {
                                setActiveProject(project.path);
                                setExpandedProjects((state) => ({ ...state, [projectKey]: true }));
                                void createSession(undefined, project.path);
                              }}
                              className="rounded-full p-1.5 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
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
                              className="rounded-full p-1.5 text-rc-text-tertiary transition-colors hover:bg-rc-accent-error-bg hover:text-rc-accent-error disabled:cursor-not-allowed disabled:opacity-30 disabled:hover:bg-transparent"
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
                            <div className="mt-3 space-y-1.5">
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
                                <div className="rounded-2xl bg-rc-bg-primary/70 px-3 py-3 text-xs text-rc-text-secondary">
                                  这个项目下还没有会话。点击右上角 + 或顶部"新会话"即可创建。
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
