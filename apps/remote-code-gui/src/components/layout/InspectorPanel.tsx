import {
  Activity,
  CheckCircle2,
  ChevronRight,
  Clock3,
  File,
  FileCode2,
  Folder,
  GitCompare,
  ListChecks,
  RefreshCw,
  ServerCog,
  TerminalSquare,
  XCircle,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { ConversationEntry, SessionSubtask, ToolProgressInfo, ToolResultInfo } from '../../lib/types';
import * as tauri from '../../lib/tauri';
import { cn, truncateMiddle } from '../../lib/utils';
import { useAppStore } from '../../stores/useAppStore';
import { useAgentStore } from '../../stores/useAgentStore';
import { useCodexStore } from '../../stores/useCodexStore';

type InspectorTab = 'tasks' | 'files' | 'diff' | 'terminal' | 'runtime';

interface FsEntry {
  fileName: string;
  isDirectory: boolean;
  isFile: boolean;
}

const inspectorTabs: Array<{ id: InspectorTab; label: string; icon: typeof ListChecks }> = [
  { id: 'tasks', label: 'Tasks', icon: ListChecks },
  { id: 'files', label: 'Files', icon: Folder },
  { id: 'diff', label: 'Diff', icon: GitCompare },
  { id: 'terminal', label: 'Terminal', icon: TerminalSquare },
  { id: 'runtime', label: 'Runtime', icon: ServerCog },
];

function parseJson(value: unknown): Record<string, unknown> | null {
  if (typeof value === 'object' && value !== null) return value as Record<string, unknown>;
  if (typeof value !== 'string') return null;
  try {
    const parsed = JSON.parse(value);
    return parsed && typeof parsed === 'object' ? (parsed as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

function summarizeToolInput(input: unknown): string {
  const parsed = parseJson(input);
  if (!parsed) return typeof input === 'string' ? truncateMiddle(input, 70) : 'tool input';
  const firstValue =
    parsed.path ?? parsed.file_path ?? parsed.command ?? parsed.query ?? parsed.prompt ?? Object.values(parsed)[0];
  return typeof firstValue === 'string' ? truncateMiddle(firstValue, 72) : 'tool input';
}

function deriveToolRows(conversation: ConversationEntry[]) {
  return conversation
    .flatMap((entry) =>
      entry.tool_calls.map((toolCall) => ({
        id: toolCall.id,
        name: toolCall.name,
        status: 'queued' as const,
        detail: summarizeToolInput(toolCall.input),
      })),
    )
    .slice(-12)
    .reverse();
}

function statusIcon(status: SessionSubtask['status']) {
  if (status === 'completed') return <CheckCircle2 size={14} className="text-rc-accent-success" />;
  if (status === 'failed') return <XCircle size={14} className="text-rc-accent-error" />;
  if (status === 'running') return <RefreshCw size={14} className="animate-spin text-rc-accent-warning" />;
  return <Clock3 size={14} className="text-rc-text-tertiary" />;
}

function InspectorHeader({
  activeTab,
  onTabChange,
}: {
  activeTab: InspectorTab;
  onTabChange: (tab: InspectorTab) => void;
}) {
  return (
    <div className="border-b border-rc-border-primary bg-rc-bg-elevated">
      <div className="flex h-11 items-center justify-between px-3">
        <div className="text-xs font-semibold uppercase text-rc-text-secondary">Inspector</div>
        <div className="text-[11px] text-rc-text-tertiary">native panes</div>
      </div>
      <div className="flex overflow-x-auto px-2 pb-2">
        {inspectorTabs.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            type="button"
            onClick={() => onTabChange(id)}
            className={cn(
              'inline-flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs font-medium transition-colors',
              activeTab === id
                ? 'bg-rc-bg-active text-rc-text-primary'
                : 'text-rc-text-tertiary hover:bg-rc-bg-hover hover:text-rc-text-primary',
            )}
          >
            <Icon size={13} />
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}

function TasksPane() {
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const sessionTasks = useAgentStore((state) => state.sessionTasks);
  const liveToolProgress = useAppStore((state) => state.liveToolProgress);
  const liveToolResults = useAppStore((state) => state.liveToolResults);
  const conversation = useAppStore((state) => state.conversation);

  const tasks = activeSessionId ? sessionTasks[activeSessionId] ?? [] : [];
  const toolRows = useMemo(() => deriveToolRows(conversation), [conversation]);

  return (
    <div className="space-y-4 p-3">
      <section>
        <div className="mb-2 flex items-center justify-between">
          <div className="text-xs font-semibold text-rc-text-secondary">Subagents</div>
          <span className="workbench-chip">{tasks.length}</span>
        </div>
        <div className="space-y-1.5">
          {tasks.length === 0 ? (
            <div className="rounded-lg border border-dashed border-rc-border-primary px-3 py-4 text-xs leading-5 text-rc-text-secondary">
              当前会话还没有子任务。启动多 agent 或后台任务后，这里会显示实时进度。
            </div>
          ) : (
            tasks.map((task) => (
              <div key={task.task_id} className="rounded-lg border border-rc-border-primary bg-rc-bg-elevated px-3 py-2">
                <div className="flex items-start gap-2">
                  <span className="mt-0.5">{statusIcon(task.status)}</span>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium text-rc-text-primary">{task.description}</div>
                    <div className="mt-1 truncate text-xs text-rc-text-tertiary">
                      {task.output_preview ?? task.summary ?? '等待输出'}
                    </div>
                  </div>
                </div>
              </div>
            ))
          )}
        </div>
      </section>

      <section>
        <div className="mb-2 flex items-center justify-between">
          <div className="text-xs font-semibold text-rc-text-secondary">Tool Stream</div>
          <span className="workbench-chip">{liveToolProgress.length + liveToolResults.length}</span>
        </div>
        <div className="space-y-1.5">
          {liveToolProgress.slice(-5).map((progress: ToolProgressInfo, index) => (
            <div key={`${progress.tool_call_id}-${index}`} className="rounded-lg bg-rc-bg-secondary px-3 py-2 text-xs">
              <div className="font-medium text-rc-text-primary">{progress.tool_name || 'tool'}</div>
              <div className="mt-1 truncate text-rc-text-secondary">{progress.active_form ?? progress.message}</div>
            </div>
          ))}
          {liveToolResults.slice(-3).map((result: ToolResultInfo, index) => (
            <div
              key={`${result.tool_call_id}-result-${index}`}
              className={cn(
                'rounded-lg px-3 py-2 text-xs',
                result.is_error ? 'bg-rc-accent-error-bg text-rc-accent-error' : 'bg-rc-accent-success-bg text-rc-accent-success',
              )}
            >
              <div className="font-medium">{result.tool_name || 'tool'}</div>
              <div className="mt-1 truncate opacity-85">{result.output}</div>
            </div>
          ))}
          {liveToolProgress.length === 0 && liveToolResults.length === 0 && toolRows.length > 0 && (
            <div className="space-y-1.5">
              {toolRows.map((row) => (
                <div key={row.id} className="rounded-lg bg-rc-bg-secondary px-3 py-2 text-xs">
                  <div className="font-medium text-rc-text-primary">{row.name}</div>
                  <div className="mt-1 truncate text-rc-text-secondary">{row.detail}</div>
                </div>
              ))}
            </div>
          )}
          {liveToolProgress.length === 0 && liveToolResults.length === 0 && toolRows.length === 0 && (
            <div className="rounded-lg border border-dashed border-rc-border-primary px-3 py-4 text-xs text-rc-text-secondary">
              工具调用会在这里按时间顺序聚合。
            </div>
          )}
        </div>
      </section>
    </div>
  );
}

function FilesPane() {
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const sessions = useAppStore((state) => state.sessions);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const activeSession = sessions.find((session) => session.id === activeSessionId) ?? null;
  const rootPath = activeSession?.cwd ?? activeProjectPath ?? '';
  const [currentPath, setCurrentPath] = useState(rootPath);
  const [entries, setEntries] = useState<FsEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setCurrentPath(rootPath);
  }, [rootPath]);

  useEffect(() => {
    let cancelled = false;
    if (!currentPath) {
      setEntries([]);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    void tauri
      .codexFsReadDirectory({ path: currentPath })
      .then((value) => {
        if (cancelled) return;
        const nextEntries = Array.isArray((value as { entries?: unknown }).entries)
          ? ((value as { entries: FsEntry[] }).entries ?? [])
          : [];
        setEntries(
          nextEntries
            .filter((entry) => !!entry && typeof entry.fileName === 'string')
            .sort((a, b) => Number(b.isDirectory) - Number(a.isDirectory) || a.fileName.localeCompare(b.fileName)),
        );
      })
      .catch((err) => {
        if (!cancelled) setError(typeof err === 'string' ? err : String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [currentPath]);

  const enterDirectory = (entry: FsEntry) => {
    if (!entry.isDirectory) return;
    const separator = currentPath.includes('\\') ? '\\' : '/';
    setCurrentPath(`${currentPath.replace(/[\\/]+$/, '')}${separator}${entry.fileName}`);
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="border-b border-rc-border-primary px-3 py-2">
        <div className="truncate font-mono text-[11px] text-rc-text-secondary" title={currentPath}>
          {currentPath || '未选择项目'}
        </div>
        <div className="mt-2 flex items-center gap-2">
          <button type="button" className="workbench-button py-1 text-xs" onClick={() => setCurrentPath(rootPath)} disabled={!rootPath}>
            Root
          </button>
          <button
            type="button"
            className="workbench-button py-1 text-xs"
            onClick={() => {
              setCurrentPath((path) => path.replace(/[\\/][^\\/]+[\\/]?$/, '') || rootPath);
            }}
            disabled={!currentPath || currentPath === rootPath}
          >
            Up
          </button>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        {loading && (
          <div className="flex items-center gap-2 px-2 py-3 text-xs text-rc-text-secondary">
            <RefreshCw size={13} className="animate-spin" />
            正在读取目录
          </div>
        )}
        {error && <div className="rounded-lg bg-rc-accent-error-bg px-3 py-2 text-xs text-rc-accent-error">{error}</div>}
        {!loading && !error && entries.length === 0 && (
          <div className="rounded-lg border border-dashed border-rc-border-primary px-3 py-4 text-xs text-rc-text-secondary">
            {currentPath ? '目录为空或不可读取。' : '选择项目后显示文件树。'}
          </div>
        )}
        {!loading &&
          !error &&
          entries.map((entry) => (
            <button
              key={entry.fileName}
              type="button"
              onClick={() => enterDirectory(entry)}
              className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary"
            >
              {entry.isDirectory ? <Folder size={14} /> : <File size={14} />}
              <span className="min-w-0 flex-1 truncate">{entry.fileName}</span>
              {entry.isDirectory && <ChevronRight size={13} />}
            </button>
          ))}
      </div>
    </div>
  );
}

function DiffPane() {
  const conversation = useAppStore((state) => state.conversation);
  const fileToolCalls = useMemo(
    () =>
      conversation
        .flatMap((entry) => entry.tool_calls)
        .filter((tool) => /edit|write|patch|apply/i.test(tool.name))
        .slice(-8)
        .reverse(),
    [conversation],
  );

  return (
    <div className="space-y-3 p-3">
      <div className="rounded-lg border border-rc-border-primary bg-rc-bg-elevated px-3 py-3">
        <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
          <GitCompare size={15} />
          Change Review
        </div>
        <p className="mt-2 text-xs leading-5 text-rc-text-secondary">
          文件编辑、patch 和写入工具会聚合在这里。具体 diff 仍使用现有权限弹窗和消息里的结构化 diff 组件。
        </p>
      </div>
      <div className="space-y-1.5">
        {fileToolCalls.length === 0 ? (
          <div className="rounded-lg border border-dashed border-rc-border-primary px-3 py-4 text-xs text-rc-text-secondary">
            当前会话还没有文件变更工具调用。
          </div>
        ) : (
          fileToolCalls.map((tool) => (
            <div key={tool.id} className="rounded-lg bg-rc-bg-secondary px-3 py-2 text-xs">
              <div className="flex items-center gap-2 font-medium text-rc-text-primary">
                <FileCode2 size={14} />
                {tool.name}
              </div>
              <div className="mt-1 truncate text-rc-text-secondary">{summarizeToolInput(tool.input)}</div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function TerminalPane() {
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);
  const sessions = useAppStore((state) => state.sessions);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const activeSession = sessions.find((session) => session.id === activeSessionId) ?? null;
  const cwd = activeSession?.cwd ?? activeProjectPath ?? null;
  const [command, setCommand] = useState('git status --short');
  const [running, setRunning] = useState(false);
  const [output, setOutput] = useState('');
  const [error, setError] = useState<string | null>(null);

  const runCommand = async () => {
    const raw = command.trim();
    if (!raw) return;
    setRunning(true);
    setError(null);
    try {
      const response = await tauri.codexExec({
        command: raw.split(/\s+/),
        cwd,
        timeoutMs: 30_000,
      });
      setOutput([response.stdout, response.stderr].filter(Boolean).join('\n') || `exit ${response.exitCode}`);
    } catch (err) {
      setError(typeof err === 'string' ? err : String(err));
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col p-3">
      <div className="rounded-lg border border-rc-border-primary bg-rc-bg-elevated p-2">
        <label className="block text-xs font-medium text-rc-text-secondary">Native command</label>
        <div className="mt-2 flex gap-2">
          <input
            value={command}
            onChange={(event) => setCommand(event.target.value)}
            className="min-w-0 flex-1 rounded-md border border-rc-border-primary bg-rc-bg-input px-2 py-1.5 font-mono text-xs text-rc-text-primary outline-none focus:border-rc-border-focus"
          />
          <button type="button" className="workbench-button py-1 text-xs" disabled={running} onClick={() => void runCommand()}>
            {running ? <RefreshCw size={13} className="animate-spin" /> : <TerminalSquare size={13} />}
            Run
          </button>
        </div>
      </div>
      {error && <div className="mt-3 rounded-lg bg-rc-accent-error-bg px-3 py-2 text-xs text-rc-accent-error">{error}</div>}
      <pre className="mt-3 min-h-0 flex-1 overflow-auto rounded-lg bg-rc-bg-code p-3 font-mono text-xs leading-5 text-rc-text-inverse">
        {output || 'Run a command through the native Codex exec bridge.'}
      </pre>
    </div>
  );
}

function RuntimePane() {
  const runtimeStatus = useAppStore((state) => state.runtimeStatus);
  const provider = useAppStore((state) => state.provider);
  const agentStatuses = useAgentStore((state) => state.agentStatuses);
  const codexRecoverableErrors = useCodexStore((state) => state.codexRecoverableErrors);

  return (
    <div className="space-y-3 p-3">
      <div className="rounded-lg border border-rc-border-primary bg-rc-bg-elevated p-3">
        <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
          <Activity size={15} />
          Runtime
        </div>
        <div className="mt-3 space-y-2 text-xs text-rc-text-secondary">
          <div>provider: {runtimeStatus?.provider.name ?? provider?.name ?? '未连接'}</div>
          <div>model: {runtimeStatus?.provider.model ?? provider?.model ?? '未配置'}</div>
          <div>protocol: {runtimeStatus?.provider.protocol ?? provider?.protocol ?? 'unknown'}</div>
          <div>permission: {runtimeStatus?.permission_mode ?? 'default'}</div>
        </div>
      </div>
      <div className="rounded-lg border border-rc-border-primary bg-rc-bg-elevated p-3">
        <div className="text-xs font-semibold text-rc-text-secondary">MCP</div>
        <div className="mt-2 grid grid-cols-2 gap-2 text-xs">
          <span className="workbench-chip">enabled {runtimeStatus?.mcp.enabled_servers ?? 0}</span>
          <span className="workbench-chip">total {runtimeStatus?.mcp.total_servers ?? 0}</span>
          <span className="workbench-chip">failed {runtimeStatus?.mcp.status_counts.failed ?? 0}</span>
          <span className="workbench-chip">auth {runtimeStatus?.mcp.status_counts.needs_auth ?? 0}</span>
        </div>
      </div>
      <div className="rounded-lg border border-rc-border-primary bg-rc-bg-elevated p-3">
        <div className="text-xs font-semibold text-rc-text-secondary">Agents</div>
        <div className="mt-2 space-y-1.5">
          {Object.keys(agentStatuses).length === 0 ? (
            <div className="text-xs text-rc-text-tertiary">暂无 agent 状态事件。</div>
          ) : (
            Object.entries(agentStatuses).map(([agent, status]) => (
              <div key={agent} className="flex items-center justify-between rounded-md bg-rc-bg-secondary px-2 py-1 text-xs">
                <span className="text-rc-text-primary">{agent}</span>
                <span className="text-rc-text-secondary">{status}</span>
              </div>
            ))
          )}
        </div>
      </div>
      {codexRecoverableErrors.length > 0 && (
        <div className="rounded-lg bg-rc-accent-warning-bg p-3 text-xs text-rc-accent-warning">
          {codexRecoverableErrors.slice(-3).map((error) => (
            <div key={`${error.session_id}-${error.timestamp}`}>{error.message}</div>
          ))}
        </div>
      )}
    </div>
  );
}

export function InspectorPanel() {
  const [activeTab, setActiveTab] = useState<InspectorTab>('tasks');

  return (
    <aside className="hidden h-full w-inspector shrink-0 flex-col border-l border-rc-border-primary bg-rc-bg-sidebar xl:flex">
      <InspectorHeader activeTab={activeTab} onTabChange={setActiveTab} />
      <div className="min-h-0 flex-1 overflow-y-auto">
        {activeTab === 'tasks' && <TasksPane />}
        {activeTab === 'files' && <FilesPane />}
        {activeTab === 'diff' && <DiffPane />}
        {activeTab === 'terminal' && <TerminalPane />}
        {activeTab === 'runtime' && <RuntimePane />}
      </div>
    </aside>
  );
}
