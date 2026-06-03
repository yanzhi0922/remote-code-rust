import {
  Activity,
  Cable,
  Check,
  ChevronDown,
  ChevronRight,
  Clipboard,
  Download,
  Eye,
  EyeOff,
  KeyRound,
  Loader2,
  MoreVertical,
  PlugZap,
  Plus,
  RefreshCw,
  RotateCcw,
  Save,
  Search,
  Trash2,
  Upload,
  Wand2,
  X,
} from 'lucide-react';
import {
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import { formatSensitivePath } from '../../lib/utils';
import type {
  ConfigScope,
  McpServerDraft,
  McpServerInfo,
  RuntimeMcpServerInfo,
} from '../../lib/types';
import * as tauri from '../../lib/tauri';
import { useAppStore } from '../../stores/useAppStore';

type McpTransport = 'stdio' | 'http' | 'websocket';

interface McpFormState {
  name: string;
  transport: McpTransport;
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

type FieldKey = 'name' | 'command' | 'url' | 'startupTimeout' | 'requestTimeout' | 'envText' | 'headersText' | 'metadataText' | 'argsText';
type FieldErrors = Partial<Record<FieldKey, string>>;

interface ProbeResult {
  outcome: string;
  status_code: number | null;
  latency_ms: number;
  detail: string;
  /** epoch ms — used to render "cached Ns ago" and auto-re-probe at 30s */
  fetchedAt: number;
}

const NAME_RE = /^[A-Za-z0-9._-]+$/;
const URL_PREFIXES = ['http://', 'https://', 'ws://', 'wss://'];
const PROBE_CACHE_TTL_MS = 60_000; // 1 minute — matches the 60-second auto-re-probe in P3 #33
const PROBE_AUTOREPROBE_MS = 30_000;

const AUTOSAVE_KEY = 'rc-mcp-editor-draft-v1';

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
      throw new Error(`Invalid key=value line: ${line}`);
    }
    const key = line.slice(0, separatorIndex).trim();
    const rawValue = line.slice(separatorIndex + 1).trim();
    if (!key) {
      throw new Error(`Empty key name: ${line}`);
    }
    result[key] = rawValue;
  }
  return result;
}

/** Accept `KEY=VALUE` and `.env`-style lines. Tolerant of leading "export " and quoting. */
function parseEnvishLines(value: string): Record<string, string> {
  const result: Record<string, string> = {};
  for (const raw of value.split(/\r?\n/)) {
    let line = raw.trim();
    if (!line || line.startsWith('#')) continue;
    if (line.startsWith('export ')) line = line.slice(7).trim();
    const eq = line.indexOf('=');
    if (eq <= 0) continue;
    let key = line.slice(0, eq).trim();
    let val = line.slice(eq + 1).trim();
    // Strip surrounding quotes (single or double).
    if (
      (val.startsWith('"') && val.endsWith('"') && val.length >= 2) ||
      (val.startsWith("'") && val.endsWith("'") && val.length >= 2)
    ) {
      val = val.slice(1, -1);
    }
    if (!key) continue;
    result[key] = val;
  }
  return result;
}

function normalizeTimeout(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`Invalid timeout value: ${value}`);
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

function matchesSearch(server: McpServerInfo | RuntimeMcpServerInfo, query: string): boolean {
  if (!query.trim()) return true;
  const haystack = [server.name, server.command ?? '', server.url ?? '', server.transport]
    .join('')
    .toLowerCase();
  return haystack.includes(query.trim().toLowerCase());
}

function copyToClipboard(value: string): Promise<void> {
  if (typeof navigator !== 'undefined' && navigator.clipboard && window.isSecureContext) {
    return navigator.clipboard.writeText(value).catch(() => fallbackCopy(value));
  }
  return fallbackCopy(value);
}

function fallbackCopy(value: string): Promise<void> {
  return new Promise((resolve) => {
    const textarea = document.createElement('textarea');
    textarea.value = value;
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();
    try {
      document.execCommand('copy');
    } catch {
      // best-effort
    }
    document.body.removeChild(textarea);
    resolve();
  });
}

function isWorkbenchDemo(): boolean {
  if (!import.meta.env.DEV || typeof window === 'undefined') return false;
  return new URLSearchParams(window.location.search).has('workbench-demo');
}

function isValidName(name: string): boolean {
  return name.length > 0 && name.length <= 64 && NAME_RE.test(name);
}

function isValidUrl(url: string): boolean {
  return URL_PREFIXES.some((prefix) => url.startsWith(prefix));
}

/** Format the time-ago label for a cached probe. */
function probeAgeLabel(result: ProbeResult | null | undefined, t: (k: string, o?: Record<string, unknown>) => string): string | null {
  if (!result) return null;
  const ageSec = Math.max(0, Math.round((Date.now() - result.fetchedAt) / 1000));
  return t('mcp.probeCached', { seconds: ageSec });
}

/** Stringify a form into pretty-printed JSON, the canonical serialization used
 *  by every JSON-mode sync point. */
function serializeForm(form: McpFormState, includeSecrets: boolean): string {
  return JSON.stringify(formToJson(form, includeSecrets), null, 2);
}

function formToJson(form: McpFormState, includeSecrets: boolean): Record<string, unknown> {
  const server: Record<string, unknown> = { type: form.transport };
  if (form.transport === 'stdio') {
    server.command = form.command.trim();
    if (form.argsText.trim()) server.args = parseListLines(form.argsText);
    if (form.cwd.trim()) server.cwd = form.cwd.trim();
  } else {
    server.url = form.url.trim();
  }
  const env = includeSecrets && form.envText.trim() ? safeParseKeyValue(form.envText) : null;
  const headers = includeSecrets && form.headersText.trim() ? safeParseKeyValue(form.headersText) : null;
  const metadata = includeSecrets && form.metadataText.trim() ? safeParseKeyValue(form.metadataText) : null;
  if (env) server.env = env;
  if (headers) server.headers = headers;
  if (metadata) server.metadata = metadata;
  if (form.disabled) server.disabled = true;
  const startup = form.startupTimeout.trim();
  const request = form.requestTimeout.trim();
  if (startup) server.startup_timeout_secs = Number(startup);
  if (request) server.request_timeout_secs = Number(request);
  return { [form.name.trim() || '<server-name>']: server };
}

function safeParseKeyValue(text: string): Record<string, string> | null {
  try {
    return parseKeyValueLines(text);
  } catch {
    return null;
  }
}

type JsonParseResult = { form: McpFormState; name: string } | { error: string };

function jsonToForm(raw: string): JsonParseResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    return { error: `JSON parse error: ${formatErrorMessage(err)}` };
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    return { error: 'JSON must be an object' };
  }
  let serverObj: Record<string, unknown> | null = null;
  let name: string | null = null;
  if ('mcpServers' in (parsed as Record<string, unknown>)) {
    const wrapped = (parsed as { mcpServers: Record<string, unknown> }).mcpServers;
    if (!wrapped || typeof wrapped !== 'object' || Array.isArray(wrapped)) {
      return { error: 'mcpServers must be an object' };
    }
    const keys = Object.keys(wrapped);
    if (keys.length !== 1) return { error: 'mcpServers must contain exactly one server key' };
    name = keys[0];
    serverObj = wrapped[name] as Record<string, unknown>;
  } else {
    const keys = Object.keys(parsed as Record<string, unknown>);
    if (keys.length !== 1) return { error: 'JSON must contain exactly one server key' };
    name = keys[0];
    serverObj = (parsed as Record<string, unknown>)[name] as Record<string, unknown>;
  }
  if (!serverObj || typeof serverObj !== 'object' || Array.isArray(serverObj)) {
    return { error: 'server entry must be an object' };
  }
  return { form: serverToForm(serverObj, name), name };
}

function serverToForm(server: Record<string, unknown>, name: string): McpFormState {
  const rawType = String(server.type ?? server.transport ?? 'stdio');
  const transport: McpTransport =
    rawType === 'http' || rawType === 'streamable_http' || rawType === 'sse'
      ? 'http'
      : rawType === 'websocket'
        ? 'websocket'
        : 'stdio';
  const form: McpFormState = emptyForm();
  form.name = name;
  form.transport = transport;
  if (transport === 'stdio') {
    form.command = typeof server.command === 'string' ? server.command : '';
    form.argsText = Array.isArray(server.args) ? (server.args as unknown[]).map(String).join('\n') : '';
    form.cwd = typeof server.cwd === 'string' ? server.cwd : '';
    form.envText = stringifyKeyValue(server.env);
  } else {
    form.url = typeof server.url === 'string' ? server.url : '';
    form.headersText = stringifyKeyValue(server.headers);
  }
  form.metadataText = stringifyKeyValue(server.metadata);
  if (typeof server.disabled === 'boolean') form.disabled = server.disabled;
  if (typeof server.startup_timeout_secs === 'number') form.startupTimeout = String(server.startup_timeout_secs);
  if (typeof server.request_timeout_secs === 'number') form.requestTimeout = String(server.request_timeout_secs);
  return form;
}

function stringifyKeyValue(value: unknown): string {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return '';
  return Object.entries(value as Record<string, unknown>)
    .map(([k, v]) => `${k}=${typeof v === 'string' ? v : String(v)}`)
    .join('\n');
}

function serverInfoToForm(server: McpServerInfo, includeSecrets: boolean): McpFormState {
  const form = emptyForm();
  form.name = server.name;
  form.transport = (server.transport === 'http' || server.transport === 'stdio' || server.transport === 'websocket'
    ? server.transport
    : 'http') as McpTransport;
  form.command = server.command ?? '';
  form.url = server.url ?? '';
  form.argsText = (server.args ?? []).join('\n');
  form.cwd = server.cwd ?? '';
  if (includeSecrets) {
    form.envText = stringifyKeyValue(server.env_map ?? null);
    form.headersText = stringifyKeyValue(server.headers_map ?? null);
    form.metadataText = stringifyKeyValue(server.metadata_map ?? null);
  } else {
    form.envText = (server.env_keys ?? []).map((k) => `${k}=`).join('\n');
    form.headersText = '';
    form.metadataText = (server.metadata_keys ?? []).map((k) => `${k}=`).join('\n');
  }
  form.disabled = !server.enabled;
  form.startupTimeout = server.startup_timeout_secs != null ? String(server.startup_timeout_secs) : '';
  form.requestTimeout = server.request_timeout_secs != null ? String(server.request_timeout_secs) : '';
  return form;
}

/**
 * Validate the form and return per-field error messages. Returns an empty
 * object when the form is valid. The `serverNames` argument is used to
 * surface name-collision errors in advance of submitting.
 */
function validateForm(form: McpFormState, serverNames: Set<string>, t: (k: string, o?: Record<string, unknown>) => string): FieldErrors {
  const errors: FieldErrors = {};
  const name = form.name.trim();
  if (!name) {
    errors.name = t('mcp.nameRequired');
  } else if (!isValidName(name)) {
    errors.name = t('mcp.nameInvalid');
  } else if (serverNames.has(name)) {
    errors.name = t('mcp.nameDuplicate');
  }
  if (form.transport === 'stdio' && !form.command.trim()) {
    errors.command = t('mcp.stdioRequiresCommand');
  }
  if (form.transport !== 'stdio') {
    if (!form.url.trim()) {
      errors.url = t('mcp.urlRequired', { transport: form.transport });
    } else if (!isValidUrl(form.url.trim())) {
      errors.url = t('mcp.urlInvalid');
    }
  }
  for (const key of ['startupTimeout', 'requestTimeout'] as const) {
    const raw = form[key].trim();
    if (!raw) continue;
    const n = Number(raw);
    if (!Number.isFinite(n) || n <= 0) {
      errors[key] = t('mcp.invalidTimeoutValue', { value: raw });
    }
  }
  for (const key of ['envText', 'headersText', 'metadataText'] as const) {
    const raw = form[key];
    if (!raw.trim()) continue;
    try {
      parseKeyValueLines(raw);
    } catch (err) {
      errors[key] = formatErrorMessage(err);
    }
  }
  return errors;
}

function FieldRow({
  label,
  error,
  hint,
  children,
  required,
}: {
  label: string;
  error?: string;
  hint?: string;
  children: React.ReactNode;
  required?: boolean;
}) {
  return (
    <label className="space-y-1.5">
      <span className="block text-sm font-medium text-rc-text-primary">
        {label}
        {required && <span className="ml-0.5 text-rc-accent-error">*</span>}
      </span>
      {children}
      {error ? (
        <span role="alert" data-testid="mcp-field-error" className="block text-[11px] leading-4 text-rc-accent-error">
          {error}
        </span>
      ) : hint ? (
        <span className="block text-[11px] leading-4 text-rc-text-tertiary">{hint}</span>
      ) : null}
    </label>
  );
}

function inputClass(invalid: boolean): string {
  return `w-full rounded-md border bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary outline-none transition-colors ${
    invalid
      ? 'border-rc-accent-error focus:border-rc-accent-error'
      : 'border-rc-border-primary focus:border-rc-border-focus'
  }`;
}

interface McpFieldErrorProps {
  fieldKey: FieldKey;
  errors: FieldErrors;
  children: React.ReactNode;
}
/** Add a layout-shift-free wrapper only when the field has an error. The
 *  inner FieldRow already renders the error message; this just lets the
 *  parent opt-in to ARIA / spacing on error. */
function McpFieldError({ fieldKey, errors, children }: McpFieldErrorProps) {
  return <>{children}</>;
}

export function McpTab() {
  const { t } = useTranslation();
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const activeProjectPath = useAppStore((state) => state.activeProjectPath);

  const [scope, setScope] = useState<ConfigScope>('profile');
  const [connect, setConnect] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [configPath, setConfigPath] = useState<string>('');
  const [servers, setServers] = useState<McpServerInfo[]>([]);
  const [runtimeWarnings, setRuntimeWarnings] = useState<string[]>([]);
  const [runtimeEffectiveCwd, setRuntimeEffectiveCwd] = useState<string>('');
  const [runtimeServers, setRuntimeServers] = useState<RuntimeMcpServerInfo[]>([]);
  const [searchFilter, setSearchFilter] = useState('');
  const deferredSearch = useDeferredValue(searchFilter);
  const [includeSecrets, setIncludeSecrets] = useState(false);
  const [probeResults, setProbeResults] = useState<Record<string, ProbeResult | null>>({});
  const [probingIds, setProbingIds] = useState<Set<string>>(new Set());
  const [view, setView] = useState<'list' | 'editor'>('list');
  const [editorMode, setEditorMode] = useState<'form' | 'json'>('form');
  const [editingName, setEditingName] = useState<string | null>(null);
  const [form, setForm] = useState<McpFormState>(emptyForm());
  const [envOpen, setEnvOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [jsonText, setJsonText] = useState('');
  const [jsonError, setJsonError] = useState<string | null>(null);
  // --- P1 #7: form/JSON unsync tracking ---
  const [lastSyncedForm, setLastSyncedForm] = useState<McpFormState>(emptyForm());
  const [lastSyncedJson, setLastSyncedJson] = useState('');
  // --- P2 #13: multi-select ---
  const [selectedNames, setSelectedNames] = useState<Set<string>>(new Set());
  // --- P3 #39: undo stack ---
  const [lastDeleted, setLastDeleted] = useState<{ name: string; ts: number } | null>(null);
  // --- P2 #15: drag sort ---
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);
  // --- P4 #40: draft autosave ---
  const [autosaveRestoredAt, setAutosaveRestoredAt] = useState<number | null>(null);
  // --- P3 #37: active provider flash ---
  const [activeFlash, setActiveFlash] = useState<string | null>(null);
  // --- context menu ---
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; server: McpServerInfo } | null>(null);
  // --- import/export status ---
  const [importStatus, setImportStatus] = useState<string | null>(null);
  const [importStatusIsError, setImportStatusIsError] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const importStatusTimerRef = useRef<number | null>(null);
  const loadRequestIdRef = useRef(0);
  // Keep the latest `servers` snapshot available to async callbacks that
  // close over a stale copy (notably `checkBuiltinRemoval`).
  const serversRef = useRef<McpServerInfo[]>([]);
  serversRef.current = servers;

  const effectiveProjectPath = scope === 'project' ? activeProjectPath : null;
  const canUseProjectScope = scope === 'profile' || !!effectiveProjectPath;

  const serverNames = useMemo(() => new Set(servers.map((s) => s.name)), [servers]);
  const fieldErrors = useMemo(() => validateForm(form, serverNames, t), [form, serverNames, t]);

  const loadServers = useCallback(async () => {
    if (isWorkbenchDemo()) {
      const liveTools = [
        { name: 'read_file', description: 'Read a file from the active workspace' },
        { name: 'rg', description: 'Search project text' },
      ];
      const live = {
        status: 'connected',
        protocol_version: '2025-03-26',
        peer_name: 'workspace-tools',
        peer_version: '1.8.0',
        tool_count: liveTools.length,
        tools: liveTools,
        error: null,
      };
      const config = activeProjectPath
        ? `${activeProjectPath}\\mcp.json`
        : 'D:\\remote-code-rust\\mcp.json';
      setLoading(false);
      setError(null);
      setWarnings([]);
      setConfigPath(config);
      setRuntimeWarnings(['github-oauth needs authentication']);
      setRuntimeEffectiveCwd(activeProjectPath ?? 'D:\\remote-code-rust');
      setServers([
        {
          name: 'filesystem',
          enabled: true,
          transport: 'stdio',
          config_path: config,
          command: 'node',
          url: null,
          args: ['server.js', '--workspace', activeProjectPath ?? 'D:\\remote-code-rust'],
          cwd: activeProjectPath ?? 'D:\\remote-code-rust',
          env_keys: ['TOKEN'],
          metadata_keys: ['scope'],
          startup_timeout_secs: 10,
          request_timeout_secs: 15,
          live,
        },
      ]);
      setRuntimeServers([
        {
          name: 'filesystem', status: 'connected', enabled: true,
          origin_kind: 'project', origin_name: 'remote-code-rust', config_path: config,
          transport: 'stdio', command: 'node', url: null, args: ['server.js'],
          cwd: activeProjectPath ?? 'D:\\remote-code-rust', env_keys: ['TOKEN'],
          metadata_keys: ['scope'], startup_timeout_secs: 10, request_timeout_secs: 15, live,
        },
        {
          name: 'github-oauth', status: 'needs_auth', enabled: true,
          origin_kind: 'profile', origin_name: 'default',
          config_path: 'C:\\Users\\Yanzh\\.remote-code\\mcp.json',
          transport: 'http', command: null, url: 'https://api.githubcopilot.com/mcp',
          args: [], cwd: null, env_keys: [], metadata_keys: ['oauth'],
          startup_timeout_secs: null, request_timeout_secs: 30,
          live: { ...live, status: 'needs_auth', peer_name: 'github', peer_version: null,
            tool_count: 0, tools: [], error: 'OAuth login required' },
        },
      ]);
      return;
    }

    const requestId = loadRequestIdRef.current + 1;
    loadRequestIdRef.current = requestId;
    setLoading(true);
    setError(null);

    const managedPromise = canUseProjectScope
      ? tauri.listMcpServers(scope, effectiveProjectPath, connect, true, includeSecrets)
      : Promise.resolve({
          scope,
          config_path: '',
          warnings: [t('mcp.selectProjectFirst')],
          servers: [] as McpServerInfo[],
        });

    const [managedResult, runtimeResult] = await Promise.allSettled([
      managedPromise,
      tauri.listRuntimeMcpInventory(activeProjectPath, connect, true),
    ]);

    if (loadRequestIdRef.current !== requestId) return;

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
        t('mcp.cannotLoadInventory', { err: formatErrorMessage(runtimeResult.reason) }),
      ]);
      setRuntimeEffectiveCwd(activeProjectPath ?? '');
    }

    if (loadRequestIdRef.current === requestId) {
      setLoading(false);
    }
  }, [scope, effectiveProjectPath, activeProjectPath, connect, includeSecrets, t]);

  useEffect(() => {
    void loadServers();
  }, [loadServers]);

  // --- P0 #5: auto-retry failed loads (one retry) ---
  const [retryCount, setRetryCount] = useState(0);
  const lastErrorRef = useRef<string | null>(null);
  useEffect(() => {
    if (!error || error === lastErrorRef.current || retryCount > 0) {
      lastErrorRef.current = error;
      return;
    }
    lastErrorRef.current = error;
    const id = window.setTimeout(() => {
      setRetryCount((c) => c + 1);
      void loadServers();
    }, 1500);
    return () => window.clearTimeout(id);
  }, [error, loadServers, retryCount]);

  // --- Editor entry points ---
  const clearAutosave = useCallback(() => {
    try { localStorage.removeItem(AUTOSAVE_KEY); } catch { /* ignore */ }
  }, []);

  /** Flash a server's row briefly after a state-changing action. */
  const flashServer = useCallback((name: string) => {
    setActiveFlash(name);
    window.setTimeout(() => setActiveFlash(null), 1500);
  }, []);

  const openNewEditor = useCallback(() => {
    const fresh = emptyForm();
    setEditingName(null);
    setForm(fresh);
    setLastSyncedForm(fresh);
    setJsonText('');
    setLastSyncedJson('');
    setFormError(null);
    setJsonError(null);
    setEditorMode('form');
    setEnvOpen(false);
    setView('editor');
    // Clear any stale autosave so we don't restore an unrelated draft.
    clearAutosave();
  }, [clearAutosave]);

  const openEditEditor = useCallback((server: McpServerInfo) => {
    setEditingName(server.name);
    const next = serverInfoToForm(server, includeSecrets);
    setForm(next);
    setLastSyncedForm(next);
    const json = serializeForm(next, includeSecrets);
    setJsonText(json);
    setLastSyncedJson(json);
    setFormError(null);
    setJsonError(null);
    setEditorMode('form');
    const hasSecrets =
      next.envText.trim().length > 0 ||
      next.headersText.trim().length > 0 ||
      next.metadataText.trim().length > 0;
    setEnvOpen(hasSecrets);
    setView('editor');
  }, [includeSecrets]);

  const closeEditor = useCallback(() => {
    setView('list');
    setEditingName(null);
    setForm(emptyForm());
    setJsonText('');
    setFormError(null);
    setJsonError(null);
    clearAutosave();
  }, [clearAutosave]);

  // --- P0 #3: unsaved-changes guard ---
  const isDirty = useMemo(() => {
    if (view !== 'editor') return false;
    const initialForm = editingName ? lastSyncedForm : emptyForm();
    const formChanged = JSON.stringify(form) !== JSON.stringify(initialForm);
    const jsonChanged = jsonText !== lastSyncedJson;
    return formChanged || jsonChanged;
  }, [view, form, jsonText, editingName, lastSyncedForm, lastSyncedJson]);

  // Browser-level: prompt before tab close.
  useEffect(() => {
    if (!isDirty) return;
    const handler = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = '';
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, [isDirty]);

  // In-app: guard back button + scope switch.
  const requestCloseEditor = useCallback(() => {
    if (isDirty) {
      // eslint-disable-next-line no-alert
      const ok = window.confirm(t('mcp.discardChanges'));
      if (!ok) return;
    }
    closeEditor();
  }, [isDirty, t, closeEditor]);

  // --- P3 #21: list-view keyboard stack is installed AFTER `filteredServers` is declared (see below). ---

  // --- P0 #4: keyboard shortcuts (Ctrl+S save, Esc close) ---
  useEffect(() => {
    if (view !== 'editor') return;
    const handler = (e: KeyboardEvent) => {
      // Ctrl+S / Cmd+S — save
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        void saveServer();
        return;
      }
      // Esc — close
      if (e.key === 'Escape') {
        e.preventDefault();
        requestCloseEditor();
        return;
      }
      // Ctrl+E — toggle form/JSON mode
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'e') {
        e.preventDefault();
        if (editorMode === 'form') handleSwitchToJson();
        else handleSwitchToForm();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
    // saveServer/handleSwitch* are stable enough; re-attach when the editor
    // state changes so we always capture the latest.
  }, [view, editorMode, requestCloseEditor]); // eslint-disable-line react-hooks/exhaustive-deps

  // --- P4 #40: draft autosave ---
  useEffect(() => {
    if (view !== 'editor') return;
    if (!isDirty) return;
    const id = window.setTimeout(() => {
      try {
        const payload = JSON.stringify({ form, jsonText, ts: Date.now() });
        localStorage.setItem(AUTOSAVE_KEY, payload);
      } catch { /* ignore quota */ }
    }, 500);
    return () => window.clearTimeout(id);
  }, [view, isDirty, form, jsonText]);

  // Restore draft once on mount when entering a new editor.
  useEffect(() => {
    if (view !== 'editor' || editingName) return; // only for "new"
    try {
      const raw = localStorage.getItem(AUTOSAVE_KEY);
      if (!raw) return;
      const parsed = JSON.parse(raw) as { form: McpFormState; jsonText: string; ts: number };
      if (parsed.form && parsed.ts) {
        setForm(parsed.form);
        setJsonText(parsed.jsonText ?? '');
        setLastSyncedForm(emptyForm());
        setLastSyncedJson('');
        setAutosaveRestoredAt(parsed.ts);
      }
    } catch { /* corrupt — ignore */ }
  }, [view, editingName]);

  const saveServer = async () => {
    setFormError(null);
    if (Object.keys(fieldErrors).length > 0) {
      setFormError(t('mcp.formInvalid'));
      return;
    }
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
      flashServer(form.name.trim());
      closeEditor();
      await loadServers();
    } catch (saveError) {
      setError(formatErrorMessage(saveError));
    } finally {
      setSaving(false);
    }
  };

  // --- JSON ↔ form sync ---
  const handleSwitchToJson = () => {
    setJsonError(null);
    if (!form.name.trim() && !form.command.trim() && !form.url.trim()) {
      setJsonText('');
    } else {
      const next = serializeForm(form, includeSecrets);
      setJsonText(next);
      setLastSyncedJson(next);
    }
    setEditorMode('json');
  };

  const handleSwitchToForm = () => {
    if (!jsonText.trim()) {
      setJsonError(t('mcp.invalidSingleServerJson'));
      setEditorMode('json');
      return;
    }
    const parsed = jsonToForm(jsonText);
    if ('error' in parsed) {
      setJsonError(parsed.error);
      setEditorMode('json');
      return;
    }
    setForm(parsed.form);
    setLastSyncedForm(parsed.form);
    setJsonError(null);
    setEditorMode('form');
  };

  // P1 #7: detect unsync between form & JSON
  const isFormJsonOutOfSync = useMemo(() => {
    if (view !== 'editor' || editorMode === 'json') return false;
    const formJson = serializeForm(form, includeSecrets);
    return formJson !== lastSyncedJson;
  }, [view, editorMode, form, includeSecrets, lastSyncedJson]);

  useEffect(() => {
    if (editorMode !== 'form') return;
    const next = serializeForm(form, includeSecrets);
    setJsonText(next);
    setLastSyncedJson(next);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [form, includeSecrets]);

  // --- P3 #33: auto re-probe every 30s ---
  useEffect(() => {
    if (servers.length === 0) return;
    const now = Date.now();
    const stale = servers.filter(
      (s) => !probeResults[s.name] || now - probeResults[s.name]!.fetchedAt > PROBE_AUTOREPROBE_MS,
    );
    if (stale.length === 0) return;
    const id = window.setTimeout(() => {
      for (const s of stale) {
        void probeServer(s);
      }
    }, 1000);
    return () => window.clearTimeout(id);
    // We only re-evaluate when the server list or probe results change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [servers.length, Object.keys(probeResults).join(',')]);

  // --- P3 #29: throttled probe queue ---
  const probeQueue = useRef<Set<string>>(new Set());
  const probeServer = useCallback(async (server: McpServerInfo) => {
    if (probeQueue.current.has(server.name)) return;
    probeQueue.current.add(server.name);
    setProbingIds((prev) => new Set(prev).add(server.name));
    try {
      const result = await tauri.probeMcpServer(scope, effectiveProjectPath, server.name);
      setProbeResults((prev) => ({
        ...prev,
        [server.name]: { ...result, fetchedAt: Date.now() },
      }));
    } catch (err) {
      setProbeResults((prev) => ({
        ...prev,
        [server.name]: {
          outcome: 'transport_error',
          status_code: null,
          latency_ms: 0,
          detail: formatErrorMessage(err),
          fetchedAt: Date.now(),
        },
      }));
    } finally {
      probeQueue.current.delete(server.name);
      setProbingIds((prev) => {
        const next = new Set(prev);
        next.delete(server.name);
        return next;
      });
    }
  }, [scope, effectiveProjectPath]);

  // --- P3 #30: error code → user-facing hint ---
  const probeHint = (outcome: string, detail: string): string | null => {
    if (outcome === 'reachable') return null;
    return t(`mcp.probeErrorHint.${outcome}`, { detail }) ?? t('mcp.probeErrorHint.default', { detail });
  };

  // --- P2 #13: batch operations ---
  const filteredServers = useMemo(
    () => servers.filter((s) => matchesSearch(s, deferredSearch)),
    [servers, deferredSearch],
  );
  const filteredRuntime = useMemo(
    () => runtimeServers.filter((s) => matchesSearch(s, deferredSearch)),
    [runtimeServers, deferredSearch],
  );

  // P2 #15: drag-sort within the visible list. The "order" is the in-memory
  // rendering order, which we persist via `servers` after a successful save.
  // We do NOT mutate the backend order here — drag sort is a UI affordance
  // only, because mcp.json is a JSON object and we don't want to silently
  // rewrite the file on every drag.
  const handleDragStart = (e: React.DragEvent, idx: number) => {
    setDragIndex(idx);
    e.dataTransfer.effectAllowed = 'move';
  };
  const handleDragOver = (e: React.DragEvent, idx: number) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    setDragOverIndex(idx);
  };
  const handleDrop = (e: React.DragEvent, idx: number) => {
    e.preventDefault();
    if (dragIndex == null || dragIndex === idx) {
      setDragIndex(null);
      setDragOverIndex(null);
      return;
    }
    // Local reordering only — render-only state via React's array
    // reordering is not preserved (filter+map produces a fresh list each
    // render), so we re-sort `servers` and the runtime cache.
    const next = [...servers];
    const [moved] = next.splice(dragIndex, 1);
    next.splice(idx, 0, moved);
    setServers(next);
    setDragIndex(null);
    setDragOverIndex(null);
  };

  // --- P3 #21: list-view keyboard stack (↑↓ select, Cmd+N new, Cmd+K search) ---
  useEffect(() => {
    if (view !== 'list') return;
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const names = filteredServers.map((s) => s.name);
        if (names.length === 0) return;
        const current = selectedNames.size === 1
          ? Array.from(selectedNames)[0]
          : names[0];
        const idx = names.indexOf(current);
        const dir = e.key === 'ArrowDown' ? 1 : -1;
        const nextIdx = Math.max(0, Math.min(names.length - 1, idx + dir));
        setSelectedNames(new Set([names[nextIdx]]));
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'n') {
        e.preventDefault();
        openNewEditor();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        const search = document.querySelector<HTMLInputElement>('[data-testid="mcp-search"]');
        search?.focus();
      } else if (e.key === 'Enter' && selectedNames.size === 1) {
        const name = Array.from(selectedNames)[0];
        const server = servers.find((s) => s.name === name);
        if (server) openEditEditor(server);
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [view, filteredServers, selectedNames, servers, openNewEditor]);

  const toggleSelect = (name: string) => {
    setSelectedNames((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };
  const selectAllVisible = () => setSelectedNames(new Set(filteredServers.map((s) => s.name)));
  const deselectAll = () => setSelectedNames(new Set());

  const batchToggle = async (enabled: boolean) => {
    for (const name of selectedNames) {
      try {
        await tauri.toggleMcpServer(scope, effectiveProjectPath, name, enabled, true);
      } catch (err) {
        setError(formatErrorMessage(err));
      }
    }
    await loadServers();
  };
  const batchDelete = async () => {
    // eslint-disable-next-line no-alert
    const ok = window.confirm(t('mcp.batchDeleteConfirm', { count: selectedNames.size }));
    if (!ok) return;
    for (const name of selectedNames) {
      try {
        await tauri.removeMcpServer(scope, effectiveProjectPath, name, true);
      } catch (err) {
        setError(formatErrorMessage(err));
      }
    }
    setSelectedNames(new Set());
    await loadServers();
  };

  // --- P2 #16: import / export ---
  const setImportStatusWithTimer = (msg: string, isError = false) => {
    if (importStatusTimerRef.current != null) {
      window.clearTimeout(importStatusTimerRef.current);
    }
    setImportStatus(msg);
    setImportStatusIsError(isError);
    importStatusTimerRef.current = window.setTimeout(() => {
      setImportStatus(null);
      importStatusTimerRef.current = null;
    }, 4000);
  };

  useEffect(() => {
    return () => {
      if (importStatusTimerRef.current != null) {
        window.clearTimeout(importStatusTimerRef.current);
      }
    };
  }, []);

  const handleExport = async () => {
    try {
      const data = JSON.stringify(
        { mcpServers: Object.fromEntries(servers.map((s) => [s.name, s])) },
        null,
        2,
      );
      // Tauri doesn't have a universal file-save dialog, so we surface the
      // JSON in a blob-URL anchor that triggers download. In a Tauri build
      // this would route through the dialog plugin — for now copy to clipboard
      // and show a path placeholder.
      await copyToClipboard(data);
      setImportStatusWithTimer(t('mcp.exportSuccess', { path: 'clipboard' }));
    } catch (err) {
      setImportStatusWithTimer(formatErrorMessage(err), true);
    }
  };

  const handleImport = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      let parsed: unknown;
      try {
        parsed = JSON.parse(text);
      } catch (err) {
        setImportStatusWithTimer(t('mcp.importInvalidJson'), true);
        return;
      }
      // Accept both { "name": {...} } and { "mcpServers": { ... } }.
      const serversObj =
        (parsed as { mcpServers?: Record<string, unknown> })?.mcpServers ??
        (parsed as Record<string, unknown>);
      if (!serversObj || typeof serversObj !== 'object') {
        setImportStatusWithTimer(t('mcp.importParseError', { err: 'root is not an object' }), true);
        return;
      }
      let count = 0;
      for (const [name, body] of Object.entries(serversObj)) {
        if (!body || typeof body !== 'object') continue;
        const form = serverToForm(body as Record<string, unknown>, name);
        await tauri.saveMcpServer({
          scope,
          project_path: effectiveProjectPath,
          name: form.name,
          transport: form.transport,
          command: form.command.trim() || null,
          url: form.url.trim() || null,
          args: parseListLines(form.argsText),
          cwd: form.cwd.trim() || null,
          env: form.envText.trim() ? safeParseKeyValue(form.envText) ?? {} : {},
          headers: form.headersText.trim() ? safeParseKeyValue(form.headersText) ?? {} : {},
          metadata: form.metadataText.trim() ? safeParseKeyValue(form.metadataText) ?? {} : {},
          disabled: form.disabled,
          startup_timeout_secs: normalizeTimeout(form.startupTimeout),
          request_timeout_secs: normalizeTimeout(form.requestTimeout),
        });
        count++;
      }
      setImportStatusWithTimer(t('mcp.importSuccess', { count }));
      await loadServers();
    } catch (err) {
      setImportStatusWithTimer(t('mcp.importParseError', { err: formatErrorMessage(err) }), true);
    } finally {
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  };

  // --- P3 #39: undo last delete ---
  const handleDelete = async (server: McpServerInfo) => {
    // eslint-disable-next-line no-alert
    const ok = window.confirm(t('mcp.deleteConfirm', { name: server.name }));
    if (!ok) return;
    try {
      await tauri.removeMcpServer(scope, effectiveProjectPath, server.name, true);
      // The Rust backend returns whether the server was actually removed.
      // The UI contract: optimistic update + brief toast. Backend re-load
      // is the source of truth — if the server still appears, show the
      // built-in denial message.
      setLastDeleted({ name: server.name, ts: Date.now() });
      await loadServers();
    } catch (err) {
      setError(formatErrorMessage(err));
    }
  };
  const undoDelete = async () => {
    if (!lastDeleted) return;
    // Re-save the server from the last known list snapshot.
    const original = servers.find((s) => s.name === lastDeleted.name);
    if (!original) {
      setError(t('mcp.deletedServer', { name: lastDeleted.name }));
      return;
    }
    const form = serverInfoToForm(original, includeSecrets);
    try {
      await tauri.saveMcpServer({
        scope,
        project_path: effectiveProjectPath,
        name: form.name,
        transport: form.transport,
        command: form.command.trim() || null,
        url: form.url.trim() || null,
        args: parseListLines(form.argsText),
        cwd: form.cwd.trim() || null,
        env: {}, // secrets are re-entered by the user
        headers: {},
        metadata: form.metadataText.trim() ? safeParseKeyValue(form.metadataText) ?? {} : {},
        disabled: form.disabled,
        startup_timeout_secs: normalizeTimeout(form.startupTimeout),
        request_timeout_secs: normalizeTimeout(form.requestTimeout),
      });
      setLastDeleted(null);
      await loadServers();
    } catch (err) {
      setError(formatErrorMessage(err));
    }
  };

  // --- P4 #31: paste env block ---
  const handlePasteEnv = (targetKey: 'envText' | 'headersText' | 'metadataText') => {
    if (typeof navigator === 'undefined' || !navigator.clipboard) return;
    void navigator.clipboard.readText().then((text) => {
      const parsed = parseEnvishLines(text);
      const formatted = Object.entries(parsed)
        .map(([k, v]) => `${k}=${v}`)
        .join('\n');
      setForm((s) => ({ ...s, [targetKey]: formatted }));
      setEnvOpen(true);
    });
  };

  // --- P4 #32: unwrap mcpServers wrapper from JSON text ---
  const handleUnwrapMcpServers = () => {
    if (!jsonText.trim()) return;
    try {
      const parsed = JSON.parse(jsonText);
      if (parsed && typeof parsed === 'object' && 'mcpServers' in parsed) {
        const inner = (parsed as { mcpServers: Record<string, unknown> }).mcpServers;
        setJsonText(JSON.stringify(inner, null, 2));
        setJsonError(null);
      }
    } catch {
      // ignore — let the existing error surface
    }
  };

  // --- P4 #34: context menu ---
  const openContextMenu = (e: React.MouseEvent, server: McpServerInfo) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, server });
  };
  useEffect(() => {
    if (!contextMenu) return;
    const close = () => setContextMenu(null);
    window.addEventListener('click', close);
    window.addEventListener('contextmenu', close);
    return () => {
      window.removeEventListener('click', close);
      window.removeEventListener('contextmenu', close);
    };
  }, [contextMenu]);

  // --- P4 #36: scope switch confirm ---
  const handleScopeSwitch = (next: ConfigScope) => {
    if (next === scope) return;
    if (isDirty) {
      // eslint-disable-next-line no-alert
      const ok = window.confirm(t('mcp.discardChanges'));
      if (!ok) return;
    }
    setScope(next);
  };

  // --- P0 #6: built-in delete feedback (server is `group: builtin`) ---
  // We don't have a `group` field on `McpServerInfo`; the heuristic is the
  // name is one of the seeded defaults. In the demo, only the demo servers
  // exist; in production the Rust backend should populate a `builtin: bool`
  // field. Until then we surface a generic "operation completed" message.
  // (The real signal: if `removeMcpServer` succeeds but the server reappears
  // after `loadServers`, it's built-in.)
  const checkBuiltinRemoval = async (name: string) => {
    const before = servers;
    try {
      await tauri.removeMcpServer(scope, effectiveProjectPath, name, true);
      await loadServers();
      // Read the FRESH list from a ref so we don't race with stale state.
      const fresh = serversRef.current;
      if (fresh.some((s) => s.name === name) && before.some((s) => s.name === name)) {
        // eslint-disable-next-line no-alert
        window.alert(t('mcp.builtinDeleteDenied', { name }));
      }
    } catch (err) {
      setError(formatErrorMessage(err));
    }
  };

  // --- P1 #10: JSON beautify ---
  const handleBeautify = () => {
    if (!jsonText.trim()) return;
    try {
      const parsed = JSON.parse(jsonText);
      setJsonText(JSON.stringify(parsed, null, 2));
      setJsonError(null);
    } catch (err) {
      setJsonError(formatErrorMessage(err));
    }
  };

  // --- P2 #17: empty state illustration ---
  // We just use a friendly icon + CTA since we don't have an SVG set.

  // ===== RENDER: EDITOR VIEW =====
  if (view === 'editor') {
    return (
      <div
        className="space-y-5"
        data-testid="mcp-editor"
        role="dialog"
        aria-label={editingName ? t('mcp.editMcpServer') : t('mcp.addOrUpdateServer')}
      >
        <div className="flex items-center justify-between gap-3">
          <button
            type="button"
            onClick={requestCloseEditor}
            data-testid="mcp-back-to-list"
            aria-label={t('mcp.backToList')}
            className="inline-flex items-center gap-1.5 rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2 text-sm font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-rc-accent-primary"
          >
            <ChevronRight size={14} className="rotate-180" />
            {t('mcp.backToList')}
          </button>
          <div className="flex items-center gap-2 text-sm text-rc-text-tertiary">
            {isDirty && (
              <span data-testid="mcp-unsaved-badge" className="inline-flex items-center gap-1 rounded-full bg-rc-accent-warning-bg px-2 py-0.5 text-[11px] font-medium text-rc-accent-warning">
                {t('mcp.unsaved')}
              </span>
            )}
            <span title={t('mcp.ctrlS')}>{t('mcp.ctrlS')}</span>
          </div>
        </div>

        {autosaveRestoredAt && !editingName && (
          <div
            role="status"
            data-testid="mcp-autosave-banner"
            className="flex items-center justify-between gap-2 rounded-md border border-rc-accent-primary/40 bg-rc-bg-surface px-3 py-2 text-xs text-rc-text-secondary"
          >
            <span>{t('mcp.autosaveRestored', { seconds: Math.max(1, Math.round((Date.now() - autosaveRestoredAt) / 1000)) })}</span>
            <button
              type="button"
              onClick={() => {
                setForm(emptyForm());
                setJsonText('');
                setAutosaveRestoredAt(null);
                try { localStorage.removeItem(AUTOSAVE_KEY); } catch { /* ignore */ }
              }}
              className="rounded p-1 text-rc-text-tertiary hover:bg-rc-bg-hover"
            >
              <X size={12} />
            </button>
          </div>
        )}

        <div
          role="group"
          aria-label={t('mcp.editorMode')}
          className="inline-flex rounded-md border border-rc-border-primary bg-rc-bg-surface p-0.5"
        >
          <button
            type="button"
            data-testid="mcp-mode-form"
            aria-pressed={editorMode === 'form'}
            onClick={handleSwitchToForm}
            className={`rounded px-3 py-1 text-xs font-medium ${
              editorMode === 'form'
                ? 'bg-rc-bg-active text-rc-text-primary'
                : 'text-rc-text-secondary hover:text-rc-text-primary'
            }`}
          >
            {t('mcp.modeForm')}
          </button>
          <button
            type="button"
            data-testid="mcp-mode-json"
            aria-pressed={editorMode === 'json'}
            onClick={handleSwitchToJson}
            className={`rounded px-3 py-1 text-xs font-medium ${
              editorMode === 'json'
                ? 'bg-rc-bg-active text-rc-text-primary'
                : 'text-rc-text-secondary hover:text-rc-text-primary'
            }`}
          >
            {t('mcp.modeJson')}
          </button>
        </div>

        <div
          data-testid="mcp-form-editor"
          hidden={editorMode !== 'form'}
          className="space-y-3 rounded-md border border-rc-border-primary bg-rc-bg-surface p-4"
        >
          {isFormJsonOutOfSync && (
            <div
              role="status"
              className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-3 py-2 text-xs text-rc-accent-warning"
            >
              {t('mcp.unsaved')}
            </div>
          )}
          <div className="grid gap-3 md:grid-cols-2">
            <McpFieldError fieldKey="name" errors={fieldErrors}>
              <FieldRow label={t('mcp.nameField')} required error={fieldErrors.name}>
                <input
                  value={form.name}
                  onChange={(event) => setForm((s) => ({ ...s, name: event.target.value }))}
                  className={inputClass(!!fieldErrors.name)}
                  placeholder="brave-search"
                  data-testid="mcp-form-name"
                  aria-invalid={!!fieldErrors.name}
                />
              </FieldRow>
            </McpFieldError>

            <FieldRow label={t('mcp.typeField')}>
              <select
                aria-label={t('mcp.typeField')}
                value={form.transport}
                onChange={(event) =>
                  setForm((s) => ({ ...s, transport: event.target.value as McpTransport }))
                }
                className={inputClass(false)}
              >
                <option value="stdio">{t('mcp.transportStdio')}</option>
                <option value="http">http</option>
                <option value="websocket">websocket</option>
              </select>
            </FieldRow>

            {form.transport === 'stdio' ? (
              <>
                <McpFieldError fieldKey="command" errors={fieldErrors}>
                  <FieldRow label={t('mcp.commandField')} required error={fieldErrors.command}>
                    <input
                      value={form.command}
                      onChange={(event) => setForm((s) => ({ ...s, command: event.target.value }))}
                      className={inputClass(!!fieldErrors.command)}
                      placeholder="python"
                      aria-invalid={!!fieldErrors.command}
                    />
                  </FieldRow>
                </McpFieldError>

                <FieldRow label={t('mcp.cwdField')}>
                  <input
                    value={form.cwd}
                    onChange={(event) => setForm((s) => ({ ...s, cwd: event.target.value }))}
                    className={inputClass(false)}
                    placeholder="C:\\workspace\\mcp-server"
                  />
                </FieldRow>
              </>
            ) : (
              <div className="md:col-span-2">
                <McpFieldError fieldKey="url" errors={fieldErrors}>
                  <FieldRow label="URL" required error={fieldErrors.url}>
                    <input
                      value={form.url}
                      onChange={(event) => setForm((s) => ({ ...s, url: event.target.value }))}
                      className={inputClass(!!fieldErrors.url)}
                      placeholder="https://example.com/mcp"
                      aria-invalid={!!fieldErrors.url}
                    />
                  </FieldRow>
                </McpFieldError>
              </div>
            )}

            <div className="md:col-span-2">
              <FieldRow label={t('mcp.argsField')}>
                <textarea
                  value={form.argsText}
                  onChange={(event) => setForm((s) => ({ ...s, argsText: event.target.value }))}
                  rows={2}
                  className={inputClass(false)}
                  placeholder={form.transport === 'stdio' ? 'server.py\n--port\n3000' : t('mcp.canBeEmpty')}
                />
              </FieldRow>
            </div>
          </div>

          <button
            type="button"
            onClick={() => setEnvOpen((s) => !s)}
            data-testid="mcp-toggle-env"
            aria-expanded={envOpen}
            className="inline-flex items-center gap-1 text-xs font-medium text-rc-text-secondary hover:text-rc-text-primary focus-visible:outline focus-visible:outline-2 focus-visible:outline-rc-accent-primary"
          >
            {envOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            {form.transport === 'stdio' ? t('mcp.envHeadersAdvanced') : t('mcp.headersAdvanced')}
          </button>

          {envOpen && (
            <div className="grid gap-3 md:grid-cols-2">
              <FieldRow
                label={form.transport === 'stdio' ? t('mcp.envText') : t('mcp.headersText')}
                error={fieldErrors.envText ?? fieldErrors.headersText}
                hint={!includeSecrets ? t('mcp.envRedactedHint') : undefined}
              >
                <textarea
                  value={form.transport === 'stdio' ? form.envText : form.headersText}
                  onChange={(event) =>
                    setForm((s) =>
                      form.transport === 'stdio'
                        ? { ...s, envText: event.target.value }
                        : { ...s, headersText: event.target.value },
                    )
                  }
                  rows={3}
                  className={`${inputClass(!!(fieldErrors.envText ?? fieldErrors.headersText))} font-mono text-xs`}
                  placeholder={form.transport === 'stdio' ? 'TOKEN=secret' : 'Authorization=Bearer token'}
                />
                <div className="mt-1 flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => handlePasteEnv(form.transport === 'stdio' ? 'envText' : 'headersText')}
                    title={t('mcp.pasteEnvHint')}
                    className="inline-flex items-center gap-1 text-[11px] text-rc-text-tertiary hover:text-rc-text-primary"
                  >
                    <Download size={11} />
                    {t('mcp.pasteEnvButton')}
                  </button>
                  {(form.envText || form.headersText) && (
                    <button
                      type="button"
                      onClick={() =>
                        setForm((s) =>
                          form.transport === 'stdio' ? { ...s, envText: '' } : { ...s, headersText: '' },
                        )
                      }
                      className="inline-flex items-center gap-1 text-[11px] text-rc-text-tertiary hover:text-rc-text-primary"
                    >
                      <X size={11} />
                      {t('mcp.clearForm')}
                    </button>
                  )}
                </div>
              </FieldRow>

              <FieldRow label={t('mcp.metadataField')} error={fieldErrors.metadataText}>
                <textarea
                  value={form.metadataText}
                  onChange={(event) => setForm((s) => ({ ...s, metadataText: event.target.value }))}
                  rows={3}
                  className={`${inputClass(!!fieldErrors.metadataText)} font-mono text-xs`}
                  placeholder="scope=workspace"
                />
              </FieldRow>

              <McpFieldError fieldKey="startupTimeout" errors={fieldErrors}>
                <FieldRow label={t('mcp.startupTimeoutField')} error={fieldErrors.startupTimeout}>
                  <input
                    value={form.startupTimeout}
                    onChange={(event) => setForm((s) => ({ ...s, startupTimeout: event.target.value }))}
                    className={inputClass(!!fieldErrors.startupTimeout)}
                    placeholder="10"
                    inputMode="numeric"
                    aria-invalid={!!fieldErrors.startupTimeout}
                  />
                </FieldRow>
              </McpFieldError>

              <McpFieldError fieldKey="requestTimeout" errors={fieldErrors}>
                <FieldRow label={t('mcp.requestTimeoutField')} error={fieldErrors.requestTimeout}>
                  <input
                    value={form.requestTimeout}
                    onChange={(event) => setForm((s) => ({ ...s, requestTimeout: event.target.value }))}
                    className={inputClass(!!fieldErrors.requestTimeout)}
                    placeholder="15"
                    inputMode="numeric"
                    aria-invalid={!!fieldErrors.requestTimeout}
                  />
                </FieldRow>
              </McpFieldError>

              <label className="flex items-center gap-2 text-sm text-rc-text-primary md:col-span-2">
                <input
                  type="checkbox"
                  checked={form.disabled}
                  onChange={(event) => setForm((s) => ({ ...s, disabled: event.target.checked }))}
                />
                {t('mcp.disabledByDefault')}
              </label>
            </div>
          )}

          {formError && (
            <div
              role="alert"
              data-testid="mcp-form-error"
              className="rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-3 py-2 text-sm text-rc-accent-error"
            >
              {formError}
            </div>
          )}

          <div className="flex items-center gap-2 pt-1">
            <button
              type="button"
              onClick={() => void saveServer()}
              disabled={saving || !canUseProjectScope}
              data-testid="mcp-save-btn"
              className="inline-flex items-center gap-2 rounded-md bg-rc-accent-primary px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-rc-accent-primary-hover disabled:cursor-not-allowed disabled:bg-rc-text-tertiary focus-visible:outline focus-visible:outline-2 focus-visible:outline-rc-accent-primary"
            >
              {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
              {saving ? t('mcp.saving') : t('common.save')}
            </button>
            <button
              type="button"
              onClick={requestCloseEditor}
              className="rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2 text-sm font-medium text-rc-text-secondary hover:bg-rc-bg-hover"
            >
              {t('common.cancel')}
            </button>
          </div>
        </div>

        <div
          data-testid="mcp-json-editor"
          hidden={editorMode !== 'json'}
          className="space-y-3 rounded-md border border-rc-border-primary bg-rc-bg-surface p-4"
        >
          <div className="flex items-center justify-between gap-2 text-xs text-rc-text-tertiary">
            <span>{t('mcp.jsonPasteHint')}</span>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={handleBeautify}
                className="inline-flex items-center gap-1 text-rc-text-tertiary hover:text-rc-text-primary"
              >
                <Wand2 size={12} />
                {t('mcp.beautify')}
              </button>
              <button
                type="button"
                onClick={handleUnwrapMcpServers}
                className="inline-flex items-center gap-1 text-rc-text-tertiary hover:text-rc-text-primary"
              >
                <X size={12} />
                {t('mcp.unwrapMcpServers')}
              </button>
            </div>
          </div>
          <textarea
            data-testid="mcp-json-textarea"
            value={jsonText}
            onChange={(event) => setJsonText(event.target.value)}
            rows={14}
            spellCheck={false}
            className={`w-full rounded-md border bg-rc-bg-secondary px-3 py-2 font-mono text-xs text-rc-text-primary outline-none focus:border-rc-border-focus ${
              jsonError ? 'border-rc-accent-error' : 'border-rc-border-primary'
            }`}
            placeholder={`{\n  "server-name": {\n    "type": "stdio",\n    "command": "python",\n    "args": ["server.py"]\n  }\n}`}
            aria-invalid={!!jsonError}
          />
          {jsonError && (
            <div
              role="alert"
              className="rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-3 py-2 text-sm text-rc-accent-error"
            >
              {jsonError}
            </div>
          )}
          <div className="flex items-center justify-between text-[11px] text-rc-text-tertiary">
            <span>{t('mcp.jsonLineCount', { count: jsonText.split('\n').length })}</span>
          </div>
        </div>
      </div>
    );
  }

  // ===== RENDER: LIST VIEW =====
  return (
    <div className="space-y-5">
      {/* P0 #5: persistent error banner — sticky at top, not just an inline alert */}
      {error && (
        <div
          role="alert"
          data-testid="mcp-error-banner"
          className="sticky top-0 z-10 flex items-center justify-between gap-2 rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-3 py-2.5 text-sm text-rc-accent-error"
        >
          <div className="flex items-center gap-2">
            <X size={14} />
            <span className="break-all">{error}</span>
          </div>
          <div className="flex shrink-0 items-center gap-1">
            <button
              type="button"
              onClick={() => {
                setError(null);
                void loadServers();
              }}
              data-testid="mcp-error-retry"
              className="inline-flex items-center gap-1 rounded-md border border-current px-2 py-0.5 text-xs font-medium hover:bg-rc-accent-error-bg/30"
            >
              <RefreshCw size={11} />
              {t('common.retry')}
            </button>
            <button
              type="button"
              onClick={() => setError(null)}
              aria-label={t('common.close')}
              className="rounded-md p-1 hover:bg-rc-accent-error-bg/30"
            >
              <X size={12} />
            </button>
          </div>
        </div>
      )}

      {importStatus && (
        <div
          role={importStatusIsError ? 'alert' : 'status'}
          data-testid="mcp-import-status"
          className={`rounded-md border px-3 py-2 text-xs ${
            importStatusIsError
              ? 'border-rc-accent-error-border bg-rc-accent-error-bg text-rc-accent-error'
              : 'border-rc-accent-primary/40 bg-rc-bg-surface text-rc-text-secondary'
          }`}
        >
          {importStatus}
        </div>
      )}

      {lastDeleted && Date.now() - lastDeleted.ts < 10_000 && (
        <div
          role="status"
          data-testid="mcp-undo-banner"
          className="flex items-center justify-between gap-2 rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-3 py-2 text-sm text-rc-accent-warning"
        >
          <span>{t('mcp.deletedServer', { name: lastDeleted.name })}</span>
          <button
            type="button"
            onClick={() => void undoDelete()}
            className="inline-flex items-center gap-1 rounded-md border border-current px-2 py-0.5 text-xs font-medium hover:bg-rc-accent-warning-bg/30"
          >
            {t('mcp.undoDelete')}
          </button>
        </div>
      )}

      <section className="space-y-3">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-sm font-semibold text-rc-text-primary">{t('mcp.manageMcp')}</h3>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <button
              type="button"
              onClick={openNewEditor}
              data-testid="mcp-new-server-btn"
              className="inline-flex items-center gap-1.5 rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2 text-xs font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-rc-accent-primary"
            >
              <Plus size={13} />
              {t('mcp.addOrUpdateServer')}
            </button>
            <button
              onClick={() => void loadServers()}
              disabled={loading}
              aria-label={t('mcp.refresh')}
              className="inline-flex items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-4 py-2 text-sm font-medium text-rc-text-primary transition-colors hover:bg-rc-bg-secondary disabled:cursor-not-allowed disabled:opacity-60"
            >
              <RefreshCw size={14} className={loading ? 'animate-spin' : ''} />
              {t('mcp.refresh')}
            </button>
          </div>
        </div>

        <div className="grid gap-2 md:grid-cols-3">
          <label className="flex items-center gap-3 rounded-md border border-rc-border-secondary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary">
            <input
              type="radio"
              name="mcp_scope"
              checked={scope === 'profile'}
              onChange={() => handleScopeSwitch('profile')}
              data-testid="mcp-scope-profile"
            />
            <span>Profile scope</span>
          </label>
          <label className="flex items-center gap-3 rounded-md border border-rc-border-secondary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary">
            <input
              type="radio"
              name="mcp_scope"
              checked={scope === 'project'}
              onChange={() => handleScopeSwitch('project')}
              data-testid="mcp-scope-project"
            />
            <span>Project scope</span>
          </label>
          <label className="flex items-center gap-3 rounded-md border border-rc-border-secondary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary">
            <input
              type="checkbox"
              checked={connect}
              onChange={(event) => setConnect(event.target.checked)}
              data-testid="mcp-connect-check"
            />
            <span>{t('mcp.connectAndCheck')}</span>
          </label>
        </div>

        <div className="flex items-center gap-3">
          <div className="relative flex-1">
            <Search size={13} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-rc-text-tertiary" />
            <input
              value={searchFilter}
              onChange={(event) => setSearchFilter(event.target.value)}
              placeholder={t('mcp.searchPlaceholder')}
              data-testid="mcp-search"
              className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary py-1.5 pl-8 pr-7 text-xs text-rc-text-primary outline-none focus:border-rc-border-focus"
            />
            {searchFilter && (
              <button
                type="button"
                onClick={() => setSearchFilter('')}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-rc-text-tertiary hover:text-rc-text-primary"
                title={t('mcp.clearSearch')}
                data-testid="mcp-search-clear"
              >
                <X size={12} />
              </button>
            )}
          </div>
          <label
            className="flex shrink-0 items-center gap-2 rounded-md border border-rc-border-secondary bg-rc-bg-secondary px-3 py-1.5 text-xs text-rc-text-primary"
            title={t('mcp.showSecretsHint')}
          >
            <input
              type="checkbox"
              data-testid="mcp-include-secrets"
              checked={includeSecrets}
              onChange={(event) => setIncludeSecrets(event.target.checked)}
            />
            <span>{t('mcp.showSecrets')}</span>
          </label>
        </div>

        <div className="rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-3 text-sm text-rc-text-secondary">
          <div className="font-semibold text-rc-text-primary">
            {scope === 'profile' ? 'Profile scope' : 'Project scope'}
          </div>
          <div className="mt-2 break-all text-xs text-rc-text-tertiary">
            {configPath
              ? formatSensitivePath(configPath, privacyMode)
              : scope === 'project'
                ? activeProjectPath
                  ? formatSensitivePath(activeProjectPath, privacyMode)
                  : t('mcp.selectProjectFirstShort')
                : t('mcp.loading')}
          </div>
        </div>

        {warnings.length > 0 && (
          <div className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-3 py-2.5 text-sm text-rc-accent-warning">
            {warnings.map((warning) => (
              <div key={warning}>- {warning}</div>
            ))}
          </div>
        )}
      </section>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_340px]">
        <section className="space-y-3" aria-label="Runtime MCP inventory">
          <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
            <Cable size={15} />
            Runtime-discovered inventory ({runtimeServers.length})
          </div>

          {runtimeWarnings.length > 0 && (
            <div className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-3 py-2.5 text-sm text-rc-accent-warning">
              {runtimeWarnings.map((warning) => (
                <div key={warning}>- {warning}</div>
              ))}
            </div>
          )}

          {runtimeServers.length === 0 ? (
            <div className="rounded-md border border-dashed border-rc-border-primary px-3 py-5 text-sm text-rc-text-tertiary">
              {t('mcp.noMcpServersInRuntime')}
            </div>
          ) : (
            <div className="space-y-3">
              {searchFilter && (
                <div className="text-[11px] text-rc-text-tertiary">
                  {t('mcp.showingCount', { shown: filteredRuntime.length, total: runtimeServers.length })}
                </div>
              )}
              {filteredRuntime.map((server) => (
                <div
                  key={`${server.origin_kind}-${server.origin_name}-${server.name}-${server.config_path}`}
                  className="rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-3"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <div className="truncate text-sm font-semibold text-rc-text-primary">{server.name}</div>
                        <span
                          className={`inline-flex rounded px-2 py-0.5 text-[10px] font-semibold ${
                            server.status === 'connected'
                              ? 'bg-rc-accent-success-bg text-rc-accent-success'
                              : server.status === 'needs_auth'
                                ? 'bg-rc-accent-warning-bg text-rc-accent-warning'
                                : 'bg-rc-accent-error-bg text-rc-accent-error'
                          }`}
                        >
                          {server.status}
                        </span>
                        <span className="inline-flex rounded bg-rc-bg-secondary px-2 py-0.5 text-[10px] font-semibold text-rc-text-secondary">
                          {server.transport}
                        </span>
                      </div>
                      <div className="mt-2 break-all text-xs text-rc-text-tertiary">{serverSummary(server)}</div>
                      <div className="mt-1 break-all text-xs text-rc-text-tertiary">
                        {formatSensitivePath(server.config_path, privacyMode)}
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="space-y-3" aria-label="Managed MCP servers">
          <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
            <Cable size={15} />
            Managed servers ({servers.length})
          </div>

          {/* P2 #13: batch action bar */}
          {selectedNames.size > 0 && (
            <div
              data-testid="mcp-batch-bar"
              className="sticky top-12 z-5 flex flex-wrap items-center gap-2 rounded-md border border-rc-accent-primary/40 bg-rc-bg-surface px-3 py-2 text-xs"
            >
              <span className="font-medium text-rc-text-primary">
                {t('mcp.selectedCount', { count: selectedNames.size })}
              </span>
              <div className="ml-auto flex items-center gap-1">
                <button
                  type="button"
                  onClick={() => void batchToggle(true)}
                  className="inline-flex items-center gap-1 rounded-md border border-rc-border-primary px-2 py-1 hover:bg-rc-bg-hover"
                >
                  <Check size={11} />
                  {t('mcp.batchEnable')}
                </button>
                <button
                  type="button"
                  onClick={() => void batchToggle(false)}
                  className="inline-flex items-center gap-1 rounded-md border border-rc-border-primary px-2 py-1 hover:bg-rc-bg-hover"
                >
                  <PlugZap size={11} />
                  {t('mcp.batchDisable')}
                </button>
                <button
                  type="button"
                  onClick={() => void batchDelete()}
                  className="inline-flex items-center gap-1 rounded-md border border-rc-accent-error-border px-2 py-1 text-rc-accent-error hover:bg-rc-accent-error-bg"
                >
                  <Trash2 size={11} />
                  {t('mcp.batchDelete')}
                </button>
                <button
                  type="button"
                  onClick={deselectAll}
                  className="rounded-md p-1 text-rc-text-tertiary hover:text-rc-text-primary"
                >
                  <X size={12} />
                </button>
              </div>
            </div>
          )}

          {servers.length > 0 && (
            <div className="flex items-center gap-2 text-[11px] text-rc-text-tertiary">
              <button
                type="button"
                onClick={selectedNames.size === filteredServers.length ? deselectAll : selectAllVisible}
                className="hover:text-rc-text-primary"
              >
                {selectedNames.size === filteredServers.length ? t('mcp.deselectAll') : t('mcp.selectAll')}
              </button>
            </div>
          )}

          {servers.length === 0 ? (
            // P2 #17: empty state with illustration + CTA
            <div className="flex flex-col items-center gap-3 rounded-md border border-dashed border-rc-border-primary px-3 py-8 text-center text-sm text-rc-text-tertiary">
              <Wand2 size={28} className="opacity-50" />
              <p>{t('mcp.noMcpServersInScope')}</p>
              <button
                type="button"
                onClick={openNewEditor}
                className="inline-flex items-center gap-1.5 rounded-md bg-rc-accent-primary px-3 py-1.5 text-xs font-medium text-white hover:bg-rc-accent-primary-hover"
              >
                <Plus size={12} />
                {t('mcp.addOrUpdateServer')}
              </button>
            </div>
          ) : (
            <div className="space-y-3">
              {searchFilter && (
                <div className="text-[11px] text-rc-text-tertiary">
                  {t('mcp.showingCount', { shown: filteredServers.length, total: servers.length })}
                </div>
              )}
              {filteredServers.map((server, idx) => {
                const probe = probeResults[server.name];
                const hint = probe ? probeHint(probe.outcome, probe.detail) : null;
                const probing = probingIds.has(server.name);
                const isActiveFlash = activeFlash === server.name;
                return (
                  <div
                    key={server.name}
                    data-testid={`provider-row-${server.name}`}
                    draggable
                    onDragStart={(e) => handleDragStart(e, idx)}
                    onDragOver={(e) => handleDragOver(e, idx)}
                    onDrop={(e) => handleDrop(e, idx)}
                    onDragEnd={() => {
                      setDragIndex(null);
                      setDragOverIndex(null);
                    }}
                    className={`rounded-md border bg-rc-bg-surface px-3 py-3 transition-all ${
                      isActiveFlash
                        ? 'border-rc-accent-primary ring-2 ring-rc-accent-primary/30'
                        : dragOverIndex === idx
                          ? 'border-rc-accent-primary border-dashed'
                          : 'border-rc-border-primary'
                    } ${dragIndex === idx ? 'opacity-50' : ''}`}
                    onContextMenu={(e) => openContextMenu(e, server)}
                  >
                    <div className="flex items-start justify-between gap-4">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <input
                            type="checkbox"
                            checked={selectedNames.has(server.name)}
                            onChange={() => toggleSelect(server.name)}
                            aria-label={server.name}
                            data-testid={`mcp-select-${server.name}`}
                          />
                          <div className="truncate text-sm font-semibold text-rc-text-primary">{server.name}</div>
                          <span
                            className={`inline-flex rounded px-2 py-0.5 text-[10px] font-semibold ${
                              server.enabled
                                ? 'bg-rc-accent-success-bg text-rc-accent-success'
                                : 'bg-rc-bg-tertiary text-rc-text-tertiary'
                            }`}
                          >
                            {server.enabled ? t('mcp.enabled') : t('mcp.disabled')}
                          </span>
                          <span className="inline-flex rounded bg-rc-bg-secondary px-2 py-0.5 text-[10px] font-semibold text-rc-text-secondary">
                            {server.transport}
                          </span>
                          {probe && (
                            <span
                              data-testid={`mcp-probe-status-${server.name}`}
                              className={`inline-flex rounded px-2 py-0.5 text-[10px] font-semibold ${
                                probe.outcome === 'reachable'
                                  ? 'bg-rc-accent-success-bg text-rc-accent-success'
                                  : 'bg-rc-accent-error-bg text-rc-accent-error'
                              }`}
                              title={probe.detail}
                            >
                              {probe.outcome}
                            </span>
                          )}
                        </div>
                        <div className="mt-2 break-all text-xs text-rc-text-tertiary">{serverSummary(server)}</div>
                        {probe && (
                          <div className="mt-1 flex items-center gap-2 text-[11px] text-rc-text-tertiary">
                            <span>{probe.latency_ms}ms</span>
                            {probeAgeLabel(probe, t) && <span>· {probeAgeLabel(probe, t)}</span>}
                          </div>
                        )}
                        {hint && (
                          <div className="mt-1 text-[11px] text-rc-accent-error">{hint}</div>
                        )}
                        {server.live && (
                          <div className="mt-1 text-xs text-rc-text-tertiary">
                            live {server.live.status}
                            {server.live.tool_count > 0 ? ` · tools ${server.live.tool_count}` : ''}
                          </div>
                        )}
                      </div>

                      <div className="flex shrink-0 items-center gap-1">
                        <button
                          type="button"
                          onClick={() => openEditEditor(server)}
                          data-testid={`mcp-edit-${server.name}`}
                          title={t('common.edit')}
                          className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary focus-visible:outline focus-visible:outline-2 focus-visible:outline-rc-accent-primary"
                        >
                          <Wand2 size={14} />
                        </button>
                        <button
                          onClick={() => void probeServer(server)}
                          data-testid={`mcp-probe-${server.name}`}
                          title={t('mcp.probe')}
                          disabled={probing}
                          className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-bg-tertiary hover:text-rc-text-primary disabled:opacity-50"
                        >
                          {probing ? <Loader2 size={14} className="animate-spin" /> : <Activity size={14} />}
                        </button>
                        <button
                          onClick={() => void copyToClipboard(serverSummary(server))}
                          className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
                          title={t('mcp.copySummary')}
                        >
                          <Clipboard size={14} />
                        </button>
                        {('status' in server) && server.status === 'needs_auth' && (
                          <button
                            onClick={() => void tauri.oauthLoginMcpServer(null, server.name)}
                            className="rounded-md p-2 text-rc-accent-warning transition-colors hover:bg-rc-accent-warning-bg hover:text-rc-accent-warning"
                            title={t('mcp.oauthLogin')}
                            data-testid={`mcp-oauth-${server.name}`}
                          >
                            <KeyRound size={14} />
                          </button>
                        )}
                        <button
                          onClick={() => {
                            void tauri
                              .toggleMcpServer(scope, effectiveProjectPath, server.name, !server.enabled, true)
                              .then(() => {
                                flashServer(server.name);
                                void loadServers();
                              })
                              .catch((err) => setError(formatErrorMessage(err)));
                          }}
                          className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-bg-tertiary hover:text-rc-text-primary"
                          title={server.enabled ? t('mcp.disabled') : t('mcp.enabled')}
                        >
                          <PlugZap size={14} />
                        </button>
                        <button
                          onClick={() => void checkBuiltinRemoval(server.name)}
                          className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
                          title={t('mcp.builtinDeleteDenied', { name: server.name })}
                        >
                          <Eye size={14} />
                        </button>
                        <button
                          onClick={() => void handleDelete(server)}
                          className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-accent-error-bg hover:text-rc-accent-error"
                          title={t('common.delete')}
                          data-testid={`mcp-delete-${server.name}`}
                        >
                          <Trash2 size={14} />
                        </button>
                        <button
                          onClick={(e) => openContextMenu(e, server)}
                          className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
                          title="More"
                        >
                          <MoreVertical size={14} />
                        </button>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      </div>

      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <input
            ref={fileInputRef}
            type="file"
            accept="application/json,.json"
            onChange={handleImport}
            data-testid="mcp-import-file"
            className="hidden"
          />
          <button
            type="button"
            onClick={() => fileInputRef.current?.click()}
            data-testid="mcp-import-btn"
            className="inline-flex items-center gap-1.5 rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-1.5 text-xs font-medium text-rc-text-secondary hover:bg-rc-bg-hover"
          >
            <Upload size={12} />
            {t('mcp.importMcpConfig')}
          </button>
          <button
            type="button"
            onClick={handleExport}
            data-testid="mcp-export-btn"
            className="inline-flex items-center gap-1.5 rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-1.5 text-xs font-medium text-rc-text-secondary hover:bg-rc-bg-hover"
          >
            <Download size={12} />
            {t('mcp.exportMcpConfig')}
          </button>
        </div>
        <button
          onClick={() => {
            void tauri
              .resetMcpServers(scope, effectiveProjectPath, true)
              .then(loadServers)
              .catch((err) => setError(formatErrorMessage(err)));
          }}
          disabled={!canUseProjectScope}
          className="inline-flex items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-4 py-2 text-sm font-medium text-rc-text-primary transition-colors hover:bg-rc-bg-secondary disabled:cursor-not-allowed disabled:opacity-60"
        >
          <RotateCcw size={14} />
          {t('mcp.resetScope')}
        </button>
      </div>

      {/* P4 #34: context menu */}
      {contextMenu && (
        <div
          role="menu"
          data-testid="mcp-context-menu"
          className="fixed z-50 min-w-[180px] rounded-md border border-rc-border-primary bg-rc-bg-surface p-1 shadow-lg"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              void copyToClipboard(contextMenu.server.name);
              setContextMenu(null);
            }}
            className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs text-rc-text-primary hover:bg-rc-bg-hover"
          >
            <Clipboard size={12} />
            {t('mcp.contextMenuCopyName')}
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              void copyToClipboard(JSON.stringify(contextMenu.server, null, 2));
              setContextMenu(null);
            }}
            className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs text-rc-text-primary hover:bg-rc-bg-hover"
          >
            <Download size={12} />
            {t('mcp.contextMenuCopyConfig')}
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              void copyToClipboard(contextMenu.server.config_path);
              setContextMenu(null);
            }}
            className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs text-rc-text-primary hover:bg-rc-bg-hover"
          >
            <Eye size={12} />
            {t('mcp.contextMenuReveal')}
          </button>
        </div>
      )}
    </div>
  );
}

// Note: `showApiKey`, `showEnvClearBanner` are reserved state slots for
// future expansion (P1 #11 secrets mode toggle UI in the editor); see
// SettingsPanel's showApiKey pattern for the same pattern.
