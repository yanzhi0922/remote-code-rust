import { Cable, PlugZap, RefreshCw, RotateCcw, Trash2 } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { formatSensitivePath } from '../../lib/utils';
import type {
  ConfigScope,
  McpServerDraft,
  McpServerInfo,
  RuntimeMcpServerInfo,
} from '../../lib/types';
import * as tauri from '../../lib/tauri';
import { useAppStore } from '../../stores/useAppStore';

interface McpFormState {
  name: string;
  transport: 'stdio' | 'http' | 'websocket';
  command: string;
  url: string;
  argsText: string;
  cwd: string;
  envText: string;
  headersText: string;
  metadataText: string;
  disabled: boolean;
  startupTimeout: string;
  requestTimeout: string;
}

function emptyForm(): McpFormState {
  return {
    name: '',
    transport: 'stdio',
    command: '',
    url: '',
    argsText: '',
    cwd: '',
    envText: '',
    headersText: '',
    metadataText: '',
    disabled: false,
    startupTimeout: '',
    requestTimeout: '',
  };
}

function parseListLines(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function parseKeyValueLines(value: string): Record<string, string> {
  const result: Record<string, string> = {};
  for (const line of parseListLines(value)) {
    const separatorIndex = line.indexOf('=');
    if (separatorIndex <= 0) {
      throw new Error(`无效的 key=value 行：${line}`);
    }
    const key = line.slice(0, separatorIndex).trim();
    const rawValue = line.slice(separatorIndex + 1).trim();
    if (!key) {
      throw new Error(`空键名：${line}`);
    }
    result[key] = rawValue;
  }
  return result;
}

function normalizeTimeout(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`无效的超时值：${value}`);
  }
  return Math.floor(parsed);
}

function formatErrorMessage(error: unknown): string {
  return typeof error === 'string'
    ? error
    : error instanceof Error
      ? error.message
      : String(error);
}

function serverSummary(
  server: Pick<McpServerInfo, 'transport' | 'command' | 'args' | 'url'>,
): string {
  if (server.transport === 'stdio') {
    return [server.command, server.args.join(' ')].filter(Boolean).join(' ');
  }
  return server.url ?? '(missing url)';
}

export function McpTab() {
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);

  const [scope, setScope] = useState<ConfigScope>('profile');
  const [connect, setConnect] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [configPath, setConfigPath] = useState<string>('');
  const [servers, setServers] = useState<McpServerInfo[]>([]);
  const [runtimeWarnings, setRuntimeWarnings] = useState<string[]>([]);
  const [runtimeEffectiveCwd, setRuntimeEffectiveCwd] = useState<string>('');
  const [runtimeServers, setRuntimeServers] = useState<RuntimeMcpServerInfo[]>([]);
  const [form, setForm] = useState<McpFormState>(emptyForm());
  const loadRequestIdRef = useRef(0);

  const effectiveProjectPath = scope === 'project' ? activeProjectPath : null;
  const canUseProjectScope = scope === 'profile' || !!effectiveProjectPath;

  const loadServers = async () => {
    const requestId = loadRequestIdRef.current + 1;
    loadRequestIdRef.current = requestId;
    setLoading(true);
    setError(null);

    const managedPromise = canUseProjectScope
      ? tauri.listMcpServers(scope, effectiveProjectPath, connect, true)
      : Promise.resolve({
          scope,
          config_path: '',
          warnings: ['请选择项目后再管理 project-scope MCP。'],
          servers: [] as McpServerInfo[],
        });

    const [managedResult, runtimeResult] = await Promise.allSettled([
      managedPromise,
      tauri.listRuntimeMcpInventory(activeProjectPath, connect, true),
    ]);

    if (loadRequestIdRef.current !== requestId) {
      return;
    }

    if (managedResult.status === 'fulfilled') {
      const managed = managedResult.value;
      setServers(managed.servers);
      setWarnings(managed.warnings);
      setConfigPath(managed.config_path);
      setError(null);
    } else {
      setServers([]);
      setWarnings([]);
      setConfigPath('');
      setError(formatErrorMessage(managedResult.reason));
    }

    if (runtimeResult.status === 'fulfilled') {
      const runtime = runtimeResult.value;
      setRuntimeServers(runtime.servers);
      setRuntimeWarnings(runtime.warnings);
      setRuntimeEffectiveCwd(runtime.effective_cwd);
    } else {
      setRuntimeServers([]);
      setRuntimeWarnings([
        `无法加载 runtime inventory：${formatErrorMessage(runtimeResult.reason)}`,
      ]);
      setRuntimeEffectiveCwd(activeProjectPath ?? '');
    }

    if (loadRequestIdRef.current === requestId) {
      setLoading(false);
    }
  };

  useEffect(() => {
    void loadServers();
    // eslint-disable-next-line react-hooks/exhaustive-deps — loadServers captures all needed state
  }, [scope, effectiveProjectPath, activeProjectPath, connect]);

  const saveServer = async () => {
    setSaving(true);
    setError(null);
    try {
      const request: McpServerDraft = {
        scope,
        project_path: effectiveProjectPath,
        name: form.name.trim(),
        transport: form.transport,
        command: form.command.trim() || null,
        url: form.url.trim() || null,
        args: parseListLines(form.argsText),
        cwd: form.cwd.trim() || null,
        env: form.envText.trim() ? parseKeyValueLines(form.envText) : {},
        headers: form.headersText.trim() ? parseKeyValueLines(form.headersText) : {},
        metadata: form.metadataText.trim() ? parseKeyValueLines(form.metadataText) : {},
        disabled: form.disabled,
        startup_timeout_secs: normalizeTimeout(form.startupTimeout),
        request_timeout_secs: normalizeTimeout(form.requestTimeout),
      };
      await tauri.saveMcpServer(request);
      setForm(emptyForm());
      await loadServers();
    } catch (saveError) {
      setError(typeof saveError === 'string' ? saveError : String(saveError));
    } finally {
      setSaving(false);
    }
  };

  const displayedScopeLabel = useMemo(
    () => (scope === 'profile' ? 'Profile scope' : 'Project scope'),
    [scope],
  );

  return (
    <div className="space-y-6">
      <section className="space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-base font-semibold text-rc-text-primary">MCP 管理</h3>
            <p className="mt-1 text-sm text-rc-text-tertiary">
              在 GUI 里直接维护 profile / project scope 的 MCP server 清单，支持连通性检查和 enable/disable。
            </p>
          </div>
          <button
            onClick={() => {
              void loadServers();
            }}
            disabled={loading}
            className="inline-flex items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-4 py-2 text-sm font-medium text-rc-text-primary transition-colors hover:bg-rc-bg-secondary disabled:cursor-not-allowed disabled:opacity-60"
          >
            <RefreshCw size={14} className={loading ? 'animate-spin' : ''} />
            刷新
          </button>
        </div>

        <div className="grid gap-3 md:grid-cols-3">
          <label className="flex items-center gap-3 rounded-md bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary">
            <input
              type="radio"
              name="mcp_scope"
              checked={scope === 'profile'}
              onChange={() => setScope('profile')}
            />
            <span>Profile scope</span>
          </label>
          <label className="flex items-center gap-3 rounded-md bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary">
            <input
              type="radio"
              name="mcp_scope"
              checked={scope === 'project'}
              onChange={() => setScope('project')}
            />
            <span>Project scope</span>
          </label>
          <label className="flex items-center gap-3 rounded-md bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary">
            <input type="checkbox" checked={connect} onChange={(event) => setConnect(event.target.checked)} />
            <span>连接并检查工具</span>
          </label>
        </div>

        <div className="rounded-lg border border-rc-border-primary bg-rc-bg-surface px-4 py-4 text-sm text-rc-text-secondary">
          <div className="font-semibold text-rc-text-primary">{displayedScopeLabel}</div>
          <div className="mt-2 break-all text-xs text-rc-text-tertiary">
            {configPath
              ? formatSensitivePath(configPath, privacyMode)
              : scope === 'project'
                ? activeProjectPath
                  ? formatSensitivePath(activeProjectPath, privacyMode)
                  : '请先选择项目'
                : '加载中…'}
          </div>
          {scope === 'project' && activeProjectPath && (
            <div className="mt-2 text-xs text-rc-text-tertiary">
              project: {formatSensitivePath(activeProjectPath, privacyMode)}
            </div>
          )}
        </div>

        {warnings.length > 0 && (
          <div className="rounded-lg border border-rc-accent-warning-border bg-rc-accent-warning-bg px-4 py-3 text-sm text-rc-accent-warning">
            {warnings.map((warning) => (
              <div key={warning}>- {warning}</div>
            ))}
          </div>
        )}
        {error && (
          <div className="rounded-lg border border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-3 text-sm text-rc-accent-error">
            {error}
          </div>
        )}
      </section>

      <section className="space-y-4 rounded-lg border border-rc-border-primary bg-rc-bg-surface p-5">
        <div className="flex items-center justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-rc-text-primary">新增 / 更新 MCP Server</div>
            <div className="mt-1 text-xs text-rc-text-tertiary">
              同名保存会直接覆盖现有 server，用来更新 transport、url、args 或 metadata。
            </div>
          </div>
          <button
            onClick={() => setForm(emptyForm())}
            className="inline-flex items-center gap-1.5 rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2 text-xs font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover"
          >
            <RotateCcw size={13} />
            清空表单
          </button>
        </div>

        <div className="grid gap-4 md:grid-cols-2">
          <label className="space-y-1.5">
            <span className="block text-sm font-medium text-rc-text-primary">名称</span>
            <input
              value={form.name}
              onChange={(event) => setForm((state) => ({ ...state, name: event.target.value }))}
              className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
              placeholder="filesystem"
            />
          </label>

          <label className="space-y-1.5">
            <span className="block text-sm font-medium text-rc-text-primary">Transport</span>
            <select
              value={form.transport}
              onChange={(event) =>
                setForm((state) => ({
                  ...state,
                  transport: event.target.value as McpFormState['transport'],
                }))
              }
              className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
            >
              <option value="stdio">stdio</option>
              <option value="http">http</option>
              <option value="websocket">websocket</option>
            </select>
          </label>

          {form.transport === 'stdio' ? (
            <>
              <label className="space-y-1.5">
                <span className="block text-sm font-medium text-rc-text-primary">Command</span>
                <input
                  value={form.command}
                  onChange={(event) => setForm((state) => ({ ...state, command: event.target.value }))}
                  className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
                  placeholder="python"
                />
              </label>

              <label className="space-y-1.5">
                <span className="block text-sm font-medium text-rc-text-primary">Working directory</span>
                <input
                  value={form.cwd}
                  onChange={(event) => setForm((state) => ({ ...state, cwd: event.target.value }))}
                  className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
                  placeholder="C:\\workspace\\mcp-server"
                />
              </label>
            </>
          ) : (
            <>
              <label className="space-y-1.5 md:col-span-2">
                <span className="block text-sm font-medium text-rc-text-primary">URL</span>
                <input
                  value={form.url}
                  onChange={(event) => setForm((state) => ({ ...state, url: event.target.value }))}
                  className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
                  placeholder="https://example.com/mcp"
                />
              </label>
            </>
          )}

          <label className="space-y-1.5 md:col-span-2">
            <span className="block text-sm font-medium text-rc-text-primary">Args / 每行一个</span>
            <textarea
              value={form.argsText}
              onChange={(event) => setForm((state) => ({ ...state, argsText: event.target.value }))}
              rows={3}
              className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
              placeholder={form.transport === 'stdio' ? 'server.py\n--port\n3000' : '可留空'}
            />
          </label>

          <label className="space-y-1.5">
            <span className="block text-sm font-medium text-rc-text-primary">
              {form.transport === 'stdio' ? 'Env (key=value)' : 'Headers (key=value)'}
            </span>
            <textarea
              value={form.transport === 'stdio' ? form.envText : form.headersText}
              onChange={(event) =>
                setForm((state) =>
                  form.transport === 'stdio'
                    ? { ...state, envText: event.target.value }
                    : { ...state, headersText: event.target.value },
                )
              }
              rows={4}
              className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
              placeholder={form.transport === 'stdio' ? 'TOKEN=secret' : 'Authorization=Bearer token'}
            />
          </label>

          <label className="space-y-1.5">
            <span className="block text-sm font-medium text-rc-text-primary">Metadata (key=value)</span>
            <textarea
              value={form.metadataText}
              onChange={(event) => setForm((state) => ({ ...state, metadataText: event.target.value }))}
              rows={4}
              className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
              placeholder="scope=workspace"
            />
          </label>

          <label className="space-y-1.5">
            <span className="block text-sm font-medium text-rc-text-primary">Startup timeout (s)</span>
            <input
              value={form.startupTimeout}
              onChange={(event) => setForm((state) => ({ ...state, startupTimeout: event.target.value }))}
              className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
              placeholder="10"
            />
          </label>

          <label className="space-y-1.5">
            <span className="block text-sm font-medium text-rc-text-primary">Request timeout (s)</span>
            <input
              value={form.requestTimeout}
              onChange={(event) => setForm((state) => ({ ...state, requestTimeout: event.target.value }))}
              className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
              placeholder="15"
            />
          </label>
        </div>

        <label className="flex items-center gap-3 rounded-md bg-rc-bg-secondary px-4 py-3 text-sm text-rc-text-primary">
          <input
            type="checkbox"
            checked={form.disabled}
            onChange={(event) => setForm((state) => ({ ...state, disabled: event.target.checked }))}
          />
          <span>保存后默认禁用</span>
        </label>

        <div className="flex flex-wrap items-center gap-3">
          <button
            onClick={() => {
              void saveServer();
            }}
            disabled={saving || !canUseProjectScope}
            className="inline-flex items-center gap-2 rounded-md bg-rc-accent-primary px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-rc-accent-primary-hover disabled:cursor-not-allowed disabled:bg-rc-text-tertiary"
          >
            <PlugZap size={14} />
            {saving ? '保存中…' : '保存 MCP Server'}
          </button>
          <button
            onClick={() => {
              void tauri
                .resetMcpServers(scope, effectiveProjectPath, true)
                .then(loadServers)
                .catch((resetError) => {
                  setError(typeof resetError === 'string' ? resetError : String(resetError));
                });
            }}
            disabled={!canUseProjectScope}
            className="inline-flex items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-4 py-2.5 text-sm font-medium text-rc-text-primary transition-colors hover:bg-rc-bg-secondary disabled:cursor-not-allowed disabled:opacity-60"
          >
            <RotateCcw size={14} />
            重置当前 scope
          </button>
        </div>
      </section>

      <section className="space-y-3">
        <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
          <Cable size={15} />
          Runtime-discovered inventory ({runtimeServers.length})
        </div>

        <div className="rounded-lg border border-rc-border-primary bg-rc-bg-surface px-4 py-4 text-sm text-rc-text-secondary">
          <div className="font-semibold text-rc-text-primary">Runtime inventory</div>
          <div className="mt-2 break-all text-xs text-rc-text-tertiary">
            cwd{' '}
            {runtimeEffectiveCwd || activeProjectPath
              ? formatSensitivePath(runtimeEffectiveCwd || activeProjectPath, privacyMode)
              : '加载中…'}
          </div>
          <div className="mt-2 text-xs text-rc-text-tertiary">
            enabled {runtimeServers.filter((server) => server.enabled).length} · disabled{' '}
            {runtimeServers.filter((server) => !server.enabled).length}
          </div>
        </div>

        {runtimeWarnings.length > 0 && (
          <div className="rounded-lg border border-rc-accent-warning-border bg-rc-accent-warning-bg px-4 py-3 text-sm text-rc-accent-warning">
            {runtimeWarnings.map((warning) => (
              <div key={warning}>- {warning}</div>
            ))}
          </div>
        )}

        {runtimeServers.length === 0 ? (
          <div className="rounded-lg border border-dashed border-rc-border-primary px-4 py-6 text-sm text-rc-text-tertiary">
            当前 runtime inventory 还没有发现 MCP server。
          </div>
        ) : (
          <div className="space-y-3">
            {runtimeServers.map((server) => (
              <div
                key={`${server.origin_kind}-${server.origin_name}-${server.name}-${server.config_path}`}
                className="rounded-lg border border-rc-border-primary bg-rc-bg-surface px-4 py-4"
              >
                <div className="flex items-start justify-between gap-4">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <div className="truncate text-sm font-semibold text-rc-text-primary">{server.name}</div>
                      <span
                        className={`inline-flex rounded-full px-2 py-0.5 text-[10px] font-semibold ${
                          server.enabled ? 'bg-rc-accent-warning-bg text-rc-accent-warning' : 'bg-rc-bg-tertiary text-rc-text-tertiary'
                        }`}
                      >
                        {server.status}
                      </span>
                      <span className="inline-flex rounded-full bg-rc-bg-secondary px-2 py-0.5 text-[10px] font-semibold text-rc-text-secondary">
                        {server.transport}
                      </span>
                      <span className="inline-flex rounded-full bg-rc-bg-active px-2 py-0.5 text-[10px] font-semibold text-rc-text-secondary">
                        {server.origin_kind}
                      </span>
                    </div>
                    <div className="mt-2 break-all text-xs text-rc-text-tertiary">{serverSummary(server)}</div>
                    <div className="mt-2 break-all text-xs text-rc-text-tertiary">
                      origin {server.origin_kind}:{server.origin_name}
                    </div>
                    <div className="mt-1 break-all text-xs text-rc-text-tertiary">
                      {formatSensitivePath(server.config_path, privacyMode)}
                    </div>
                    {server.live && (
                      <div className="mt-2 text-xs text-rc-text-tertiary">
                        live {server.live.status}
                        {server.live.tool_count > 0 ? ` · tools ${server.live.tool_count}` : ''}
                        {server.live.peer_name ? ` · ${server.live.peer_name}` : ''}
                        {server.live.peer_version ? ` ${server.live.peer_version}` : ''}
                      </div>
                    )}
                    {server.live?.error && (
                      <div className="mt-2 text-xs text-rc-accent-error">{server.live.error}</div>
                    )}
                    {server.live && server.live.tools.length > 0 && (
                      <div className="mt-3 flex flex-wrap gap-1.5">
                        {server.live.tools.map((tool) => (
                          <span
                            key={`${server.origin_kind}-${server.name}-${tool.name}`}
                            className="inline-flex rounded-full bg-rc-bg-tertiary px-2 py-1 text-[11px] text-rc-text-secondary"
                            title={tool.description ?? tool.name}
                          >
                            {tool.name}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="space-y-3">
        <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
          <Cable size={15} />
          Managed servers ({servers.length})
        </div>

        {servers.length === 0 ? (
          <div className="rounded-lg border border-dashed border-rc-border-primary px-4 py-6 text-sm text-rc-text-tertiary">
            当前 scope 下还没有 MCP server。保存上面的表单后，这里会立刻刷新。
          </div>
        ) : (
          <div className="space-y-3">
            {servers.map((server) => (
              <div key={server.name} className="rounded-lg border border-rc-border-primary bg-rc-bg-surface px-4 py-4">
                <div className="flex items-start justify-between gap-4">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <div className="truncate text-sm font-semibold text-rc-text-primary">{server.name}</div>
                      <span
                        className={`inline-flex rounded-full px-2 py-0.5 text-[10px] font-semibold ${
                          server.enabled ? 'bg-rc-accent-success-bg text-rc-accent-success' : 'bg-rc-bg-tertiary text-rc-text-tertiary'
                        }`}
                      >
                        {server.enabled ? 'enabled' : 'disabled'}
                      </span>
                      <span className="inline-flex rounded-full bg-rc-bg-secondary px-2 py-0.5 text-[10px] font-semibold text-rc-text-secondary">
                        {server.transport}
                      </span>
                    </div>
                    <div className="mt-2 break-all text-xs text-rc-text-tertiary">{serverSummary(server)}</div>
                    {server.live && (
                      <div className="mt-2 text-xs text-rc-text-tertiary">
                        live {server.live.status}
                        {server.live.tool_count > 0 ? ` · tools ${server.live.tool_count}` : ''}
                        {server.live.peer_name ? ` · ${server.live.peer_name}` : ''}
                        {server.live.peer_version ? ` ${server.live.peer_version}` : ''}
                      </div>
                    )}
                    {server.live?.error && (
                      <div className="mt-2 text-xs text-rc-accent-error">{server.live.error}</div>
                    )}
                    {server.live && server.live.tools.length > 0 && (
                      <div className="mt-3 flex flex-wrap gap-1.5">
                        {server.live.tools.map((tool) => (
                          <span
                            key={`${server.name}-${tool.name}`}
                            className="inline-flex rounded-full bg-rc-bg-tertiary px-2 py-1 text-[11px] text-rc-text-secondary"
                            title={tool.description ?? tool.name}
                          >
                            {tool.name}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>

                  <div className="flex shrink-0 items-center gap-2">
                    <button
                      onClick={() => {
                        void tauri
                          .toggleMcpServer(scope, effectiveProjectPath, server.name, !server.enabled, true)
                          .then(loadServers)
                          .catch((toggleError) => {
                            setError(typeof toggleError === 'string' ? toggleError : String(toggleError));
                          });
                      }}
                      className="rounded-full p-2 text-slate-400 transition-colors hover:bg-rc-bg-tertiary hover:text-rc-text-primary"
                      title={server.enabled ? 'Disable server' : 'Enable server'}
                    >
                      <PlugZap size={15} />
                    </button>
                    <button
                      onClick={() => {
                        void tauri
                          .removeMcpServer(scope, effectiveProjectPath, server.name, true)
                          .then(loadServers)
                          .catch((removeError) => {
                            setError(typeof removeError === 'string' ? removeError : String(removeError));
                          });
                      }}
                      className="rounded-full p-2 text-slate-400 transition-colors hover:bg-rc-accent-error-bg hover:text-rc-accent-error"
                      title="Remove server"
                    >
                      <Trash2 size={15} />
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
