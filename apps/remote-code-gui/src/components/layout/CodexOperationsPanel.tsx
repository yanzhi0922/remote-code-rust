import { Bot, Code2, Database, GitFork, MemoryStick, PlugZap, RefreshCw, Send, TerminalSquare } from 'lucide-react';
import type { ReactNode } from 'react';
import { useMemo, useState } from 'react';
import * as tauri from '../../lib/tauri';
import type { CodexThreadListResponse, CodexThreadSummary } from '../../lib/types';

type JsonValue = object | unknown[] | string | number | boolean | null;
type ThreadSummary = CodexThreadSummary;

function asError(error: unknown): string {
  return typeof error === 'string' ? error : error instanceof Error ? error.message : String(error);
}

function prettyJson(value: JsonValue): string {
  return JSON.stringify(value, null, 2);
}

function getThreads(value: JsonValue): ThreadSummary[] {
  if (!value || typeof value !== 'object') return [];
  const data = (value as Partial<CodexThreadListResponse>).data;
  if (!Array.isArray(data)) return [];
  return data.filter((item): item is ThreadSummary => {
    return !!item && typeof item === 'object' && typeof (item as ThreadSummary).id === 'string';
  });
}

function formatTime(seconds?: number): string {
  if (!seconds) return 'unknown';
  return new Date(seconds * 1000).toLocaleString();
}

function JsonBlock({ value }: { value: JsonValue }) {
  if (value == null) return null;
  return (
    <pre className="max-h-80 overflow-auto rounded-2xl bg-[#151515] px-4 py-3 text-xs leading-relaxed text-[#ece7da]">
      {prettyJson(value)}
    </pre>
  );
}

function confirmRisk(message: string): boolean {
  return window.confirm(message);
}

function Section({
  title,
  description,
  icon: Icon,
  children,
}: {
  title: string;
  description: string;
  icon: typeof Bot;
  children: ReactNode;
}) {
  return (
    <section className="space-y-4 rounded-[28px] border border-[#ddd6c8] bg-white px-5 py-5">
      <div className="flex items-start gap-3">
        <div className="rounded-2xl bg-[#f1eadf] p-2 text-slate-700">
          <Icon size={18} />
        </div>
        <div>
          <h3 className="text-base font-semibold text-slate-800">{title}</h3>
          <p className="mt-1 text-sm text-slate-500">{description}</p>
        </div>
      </div>
      {children}
    </section>
  );
}

export function CodexOperationsPanel() {
  const [threadsResponse, setThreadsResponse] = useState<JsonValue>(null);
  const [threadDetail, setThreadDetail] = useState<JsonValue>(null);
  const [selectedThreadId, setSelectedThreadId] = useState('');
  const [includeTurns, setIncludeTurns] = useState(true);
  const [searchTerm, setSearchTerm] = useState('');
  const [includeArchived, setIncludeArchived] = useState(false);

  const [mcpResponse, setMcpResponse] = useState<JsonValue>(null);
  const [mcpServer, setMcpServer] = useState('');
  const [mcpResourceUri, setMcpResourceUri] = useState('');
  const [mcpTool, setMcpTool] = useState('');
  const [mcpArgs, setMcpArgs] = useState('{}');

  const [configResponse, setConfigResponse] = useState<JsonValue>(null);
  const [configKey, setConfigKey] = useState('');
  const [configValue, setConfigValue] = useState('');

  const [execCommand, setExecCommand] = useState('codex --version');
  const [execResponse, setExecResponse] = useState<JsonValue>(null);

  const [appServerMethod, setAppServerMethod] = useState('model/list');
  const [appServerParams, setAppServerParams] = useState('{}');
  const [appServerResponse, setAppServerResponse] = useState<JsonValue>(null);

  const [feedbackThreadId, setFeedbackThreadId] = useState('');
  const [feedbackReason, setFeedbackReason] = useState('');
  const [feedbackResponse, setFeedbackResponse] = useState<JsonValue>(null);
  const [memoryEnabled, setMemoryEnabled] = useState(true);
  const [memoryResponse, setMemoryResponse] = useState<JsonValue>(null);

  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const threads = useMemo(() => getThreads(threadsResponse), [threadsResponse]);

  async function run(label: string, fn: () => Promise<JsonValue>) {
    setBusy(label);
    setError(null);
    try {
      return await fn();
    } catch (err) {
      setError(asError(err));
      return null;
    } finally {
      setBusy(null);
    }
  }

  async function refreshThreads() {
    const response = await run('threads', () =>
      tauri.codexListThreads({
        limit: 20,
        sortKey: 'updatedAt',
        sortDirection: 'desc',
        archived: includeArchived ? null : false,
        searchTerm: searchTerm.trim() || null,
      }),
    );
    setThreadsResponse(response);
    const nextThreads = getThreads(response);
    if (!selectedThreadId && nextThreads[0]) {
      setSelectedThreadId(nextThreads[0].id);
    }
  }

  async function operateThread(action: 'read' | 'resume' | 'fork' | 'archive' | 'unarchive') {
    if (!selectedThreadId) return;
    if (
      action === 'archive' &&
      !confirmRisk(`Archive Codex thread ${selectedThreadId}? It will be hidden from the active thread list.`)
    ) {
      return;
    }
    if (
      action === 'unarchive' &&
      !confirmRisk(`Unarchive Codex thread ${selectedThreadId}? It will reappear in active thread lists.`)
    ) {
      return;
    }
    const request = { threadId: selectedThreadId, includeTurns };
    const response = await run(action, () => {
      if (action === 'read') return tauri.codexReadThread(request);
      if (action === 'resume') return tauri.codexResumeThread(request);
      if (action === 'fork') return tauri.codexForkThread(request);
      if (action === 'archive') return tauri.codexArchiveThread({ threadId: selectedThreadId });
      return tauri.codexUnarchiveThread({ threadId: selectedThreadId });
    });
    setThreadDetail(response);
  }

  async function refreshMcp() {
    const response = await run('mcp-refresh', async () => {
      await tauri.codexMcpRefresh();
      return tauri.codexMcpStatus({ detail: 'full', limit: 50 });
    });
    setMcpResponse(response);
  }

  async function readResource() {
    if (!mcpServer || !mcpResourceUri) return;
    const response = await run('mcp-resource', () =>
      tauri.codexMcpReadResource({ server: mcpServer, uri: mcpResourceUri }),
    );
    setMcpResponse(response);
  }

  async function callTool() {
    if (!mcpServer || !mcpTool) return;
    if (!selectedThreadId) {
      setError('Select a Codex thread before calling MCP tools.');
      return;
    }
    let parsedArgs: unknown = {};
    try {
      parsedArgs = JSON.parse(mcpArgs || '{}');
    } catch {
      setError('MCP tool arguments must be valid JSON.');
      return;
    }
    const response = await run('mcp-tool', () =>
      tauri.codexMcpCallTool({
        sessionId: null,
        threadId: selectedThreadId,
        server: mcpServer,
        tool: mcpTool,
        arguments: parsedArgs,
      }),
    );
    setMcpResponse(response);
  }

  async function readConfig() {
    const response = await run('config-read', () => tauri.codexReadConfig(true));
    setConfigResponse(response);
  }

  async function writeConfig() {
    if (!configKey.trim()) return;
    let parsed: unknown = configValue;
    try {
      parsed = JSON.parse(configValue);
    } catch {
      parsed = configValue;
    }
    if (
      !confirmRisk(
        `Write Codex config "${configKey.trim()}"? This changes the effective Codex configuration and may affect future runs.`,
      )
    ) {
      return;
    }
    const response = await run('config-write', () =>
      tauri.codexWriteConfigValue({
        keyPath: configKey.trim(),
        value: parsed,
        mergeStrategy: 'upsert',
      }),
    );
    setConfigResponse(response);
  }

  async function runExec() {
    const command = execCommand.trim();
    if (!command) return;
    if (
      !confirmRisk(
        `Run this command through Codex exec?\n\n${command}\n\nCommands can modify files or system state.`,
      )
    ) {
      return;
    }
    const response = await run('exec', () =>
      tauri.codexExec({
        command: ['powershell', '-NoProfile', '-Command', command],
        timeoutMs: 30000,
        streamStdoutStderr: false,
      }),
    );
    setExecResponse(response);
  }

  async function runAppServerRequest() {
    const method = appServerMethod.trim();
    if (!method) return;
    let parsedParams: unknown = {};
    try {
      parsedParams = JSON.parse(appServerParams || '{}');
    } catch {
      setError('Codex app-server params must be valid JSON.');
      return;
    }
    if (
      !confirmRisk(
        `Send Codex app-server request "${method}"? Some methods can mutate threads, config, files, or sandbox setup.`,
      )
    ) {
      return;
    }
    const response = await run('app-server-request', () =>
      tauri.codexAppServerRequest({
        method,
        params: parsedParams,
      }),
    );
    setAppServerResponse(response);
  }

  async function setMemoryMode() {
    const threadId = feedbackThreadId.trim() || selectedThreadId;
    if (!threadId) return;
    const response = await run('memory-mode', () =>
      tauri.codexSetThreadMemoryMode({ threadId, enabled: memoryEnabled }),
    );
    setMemoryResponse(response);
  }

  async function resetMemories() {
    if (
      !confirmRisk(
        'Reset all Codex memories? This removes stored memory context and cannot be undone from this UI.',
      )
    ) {
      return;
    }
    const response = await run('memory-reset', () => tauri.codexResetMemories());
    setMemoryResponse(response);
  }

  async function uploadFeedback() {
    const response = await run('feedback', () =>
      tauri.codexUploadFeedback({
        classification: 'user_report',
        reason: feedbackReason.trim() || null,
        threadId: feedbackThreadId.trim() || selectedThreadId || null,
        includeLogs: true,
      }),
    );
    setFeedbackResponse(response);
  }

  return (
    <div className="space-y-6" data-testid="codex-operations">
      <div className="rounded-[32px] border border-[#d8cfbf] bg-[#17181a] px-5 py-5 text-white">
        <div className="flex items-start gap-3">
          <div className="rounded-2xl bg-white/10 p-2">
            <Bot size={20} />
          </div>
          <div>
            <h2 className="text-lg font-semibold">Codex Native Control Surface</h2>
            <p className="mt-1 text-sm text-white/70">
              直接调用官方 Codex app-server 能力：thread store、resume/fork、MCP、config、exec、memory 和 feedback。
            </p>
          </div>
        </div>
      </div>

      {error && (
        <div className="rounded-[24px] border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {error}
        </div>
      )}

      <Section
        title="Thread store / Resume / Fork"
        description="列出隔离 Codex thread store 中的线程，并执行官方 resume、fork、archive 操作。"
        icon={GitFork}
      >
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_180px_140px]">
          <input
            value={searchTerm}
            onChange={(event) => setSearchTerm(event.target.value)}
            placeholder="搜索 thread preview/title"
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 text-sm"
          />
          <label className="flex items-center gap-2 rounded-2xl border border-[#ddd6c8] px-3 py-2 text-sm text-slate-600">
            <input
              type="checkbox"
              checked={includeArchived}
              onChange={(event) => setIncludeArchived(event.target.checked)}
            />
            包含 archived
          </label>
          <button
            type="button"
            onClick={() => void refreshThreads()}
            disabled={busy === 'threads'}
            className="inline-flex items-center justify-center gap-2 rounded-2xl bg-[#17181a] px-4 py-2 text-sm font-medium text-white disabled:opacity-60"
          >
            <RefreshCw size={14} className={busy === 'threads' ? 'animate-spin' : ''} />
            刷新
          </button>
        </div>

        {threads.length > 0 && (
          <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_220px]">
            <div className="space-y-2">
              {threads.map((thread) => (
                <button
                  key={thread.id}
                  type="button"
                  onClick={() => {
                    setSelectedThreadId(thread.id);
                    setFeedbackThreadId(thread.id);
                  }}
                  className={`w-full rounded-2xl border px-4 py-3 text-left transition-colors ${
                    selectedThreadId === thread.id
                      ? 'border-slate-800 bg-[#f4efe5]'
                      : 'border-[#ddd6c8] bg-white hover:bg-[#faf8f3]'
                  }`}
                >
                  <div className="truncate text-sm font-semibold text-slate-800">
                    {thread.name || thread.preview || thread.id}
                  </div>
                  <div className="mt-1 truncate text-xs text-slate-500">{thread.id}</div>
                  <div className="mt-1 text-xs text-slate-400">
                    {thread.modelProvider ?? 'provider'} · {formatTime(thread.updatedAt)}
                  </div>
                </button>
              ))}
            </div>
            <div className="space-y-2">
              <label className="flex items-center gap-2 rounded-2xl border border-[#ddd6c8] px-3 py-2 text-sm text-slate-600">
                <input
                  type="checkbox"
                  checked={includeTurns}
                  onChange={(event) => setIncludeTurns(event.target.checked)}
                />
                include turns
              </label>
              {(['read', 'resume', 'fork', 'archive', 'unarchive'] as const).map((action) => (
                <button
                  key={action}
                  type="button"
                  onClick={() => void operateThread(action)}
                  disabled={!selectedThreadId || busy === action}
                  className="w-full rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-[#faf8f3] disabled:opacity-60"
                >
                  {action}
                </button>
              ))}
            </div>
          </div>
        )}

        <JsonBlock value={threadDetail ?? threadsResponse} />
      </Section>

      <Section
        title="MCP servers"
        description="刷新 Codex MCP server 状态，读取资源，或在已选 Codex thread 上调用某个 MCP tool。"
        icon={PlugZap}
      >
        <div className="flex flex-wrap gap-3">
          <button
            type="button"
            onClick={() => void refreshMcp()}
            className="inline-flex items-center gap-2 rounded-2xl bg-[#17181a] px-4 py-2 text-sm font-medium text-white"
          >
            <RefreshCw size={14} />
            刷新 MCP
          </button>
        </div>
        <div className="grid gap-3 lg:grid-cols-3">
          <input
            value={mcpServer}
            onChange={(event) => setMcpServer(event.target.value)}
            placeholder="server name"
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 text-sm"
          />
          <input
            value={mcpResourceUri}
            onChange={(event) => setMcpResourceUri(event.target.value)}
            placeholder="resource URI"
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 text-sm"
          />
          <button
            type="button"
            onClick={() => void readResource()}
            className="rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2 text-sm font-medium text-slate-700"
          >
            read resource
          </button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[180px_180px_minmax(0,1fr)_130px]">
          <input
            value={mcpServer}
            onChange={(event) => setMcpServer(event.target.value)}
            placeholder="server name"
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 text-sm"
          />
          <input
            value={mcpTool}
            onChange={(event) => setMcpTool(event.target.value)}
            placeholder="tool name"
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 text-sm"
          />
          <input
            value={mcpArgs}
            onChange={(event) => setMcpArgs(event.target.value)}
            placeholder='{"key":"value"}'
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 font-mono text-xs"
          />
          <button
            type="button"
            onClick={() => void callTool()}
            className="rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2 text-sm font-medium text-slate-700"
          >
            call tool
          </button>
        </div>
        <JsonBlock value={mcpResponse} />
      </Section>

      <Section
        title="Codex config"
        description="读取官方 Codex effective config/layers，或写入单个 config value。"
        icon={Database}
      >
        <div className="flex flex-wrap gap-3">
          <button
            type="button"
            onClick={() => void readConfig()}
            className="rounded-2xl bg-[#17181a] px-4 py-2 text-sm font-medium text-white"
          >
            read config + layers
          </button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[220px_minmax(0,1fr)_130px]">
          <input
            value={configKey}
            onChange={(event) => setConfigKey(event.target.value)}
            placeholder="key.path"
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 text-sm"
          />
          <input
            value={configValue}
            onChange={(event) => setConfigValue(event.target.value)}
            placeholder='"value", true, 123, or JSON'
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 font-mono text-xs"
          />
          <button
            type="button"
            onClick={() => void writeConfig()}
            className="rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2 text-sm font-medium text-slate-700"
          >
            write
          </button>
        </div>
        <JsonBlock value={configResponse} />
      </Section>

      <Section
        title="Non-interactive exec"
        description="通过官方 command/exec API 执行非交互命令，用于补齐 codex exec 类能力。"
        icon={Code2}
      >
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_130px]">
          <input
            value={execCommand}
            onChange={(event) => setExecCommand(event.target.value)}
            placeholder="codex --version"
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 font-mono text-xs"
          />
          <button
            type="button"
            onClick={() => void runExec()}
            className="inline-flex items-center justify-center gap-2 rounded-2xl bg-[#17181a] px-4 py-2 text-sm font-medium text-white"
          >
            <Send size={14} />
            run
          </button>
        </div>
        <JsonBlock value={execResponse} />
      </Section>

      <Section
        title="Official app-server API"
        description="薄透传官方 ClientRequest：可调用 thread/compact/start、thread/goal/*、turn/steer、model/list、skills/list、plugin/list、fs/*、windowsSandbox/setupStart 等。"
        icon={TerminalSquare}
      >
        <div className="grid gap-3 lg:grid-cols-[260px_minmax(0,1fr)_150px]">
          <input
            value={appServerMethod}
            onChange={(event) => setAppServerMethod(event.target.value)}
            placeholder="model/list"
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 font-mono text-xs"
          />
          <input
            value={appServerParams}
            onChange={(event) => setAppServerParams(event.target.value)}
            placeholder='{"threadId":"..."}'
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 font-mono text-xs"
          />
          <button
            type="button"
            onClick={() => void runAppServerRequest()}
            className="inline-flex items-center justify-center gap-2 rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2 text-sm font-medium text-slate-700"
          >
            request
          </button>
        </div>
        <div className="rounded-2xl bg-[#f7f5ef] px-4 py-3 text-xs leading-5 text-slate-600">
          {'常用示例：`model/list` 用 `{}`；`thread/compact/start` 用 `{"threadId":"..."}`；`thread/goal/get` 用 `{"threadId":"..."}`；`skills/list` 用 `{"cwds":[]}`。'}
        </div>
        <JsonBlock value={appServerResponse} />
      </Section>

      <Section
        title="Memory / Feedback"
        description="设置线程 memory mode、清空 Codex memories，并上传包含日志的 Codex feedback。"
        icon={MemoryStick}
      >
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_160px_140px_150px]">
          <input
            value={feedbackThreadId}
            onChange={(event) => setFeedbackThreadId(event.target.value)}
            placeholder="thread id"
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 text-sm"
          />
          <label className="flex items-center gap-2 rounded-2xl border border-[#ddd6c8] px-3 py-2 text-sm text-slate-600">
            <input
              type="checkbox"
              checked={memoryEnabled}
              onChange={(event) => setMemoryEnabled(event.target.checked)}
            />
            memory enabled
          </label>
          <button
            type="button"
            onClick={() => void setMemoryMode()}
            className="rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2 text-sm font-medium text-slate-700"
          >
            set mode
          </button>
          <button
            type="button"
            onClick={() => void resetMemories()}
            className="rounded-2xl border border-rose-200 bg-rose-50 px-3 py-2 text-sm font-medium text-rose-700"
          >
            reset memories
          </button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_150px]">
          <input
            value={feedbackReason}
            onChange={(event) => setFeedbackReason(event.target.value)}
            placeholder="feedback reason"
            className="rounded-2xl border border-[#ddd6c8] px-3 py-2 text-sm"
          />
          <button
            type="button"
            onClick={() => void uploadFeedback()}
            className="rounded-2xl bg-[#17181a] px-4 py-2 text-sm font-medium text-white"
          >
            upload feedback
          </button>
        </div>
        <JsonBlock value={memoryResponse ?? feedbackResponse} />
      </Section>
    </div>
  );
}
