import {
  Boxes,
  Compass,
  Database,
  Flag,
  GitFork,
  LogIn,
  MemoryStick,
  PlugZap,
  RefreshCw,
  RotateCcw,
  Send,
  ShieldAlert,
  Square,
  TerminalSquare,
} from 'lucide-react';
import type { ReactNode } from 'react';
import { useMemo, useState } from 'react';
import * as tauri from '../../lib/tauri';
import type { CodexJsonValue, CodexThreadListResponse, CodexThreadSummary } from '../../lib/types';

type ThreadSummary = CodexThreadSummary;

function asError(error: unknown): string {
  return typeof error === 'string' ? error : error instanceof Error ? error.message : String(error);
}

function prettyJson(value: CodexJsonValue): string {
  return JSON.stringify(value, null, 2);
}

function getThreads(value: CodexJsonValue): ThreadSummary[] {
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

function JsonBlock({ value }: { value: CodexJsonValue }) {
  if (value == null) return null;
  return (
    <pre className="max-h-80 overflow-auto rounded-md bg-rc-bg-code px-4 py-3 text-xs leading-relaxed text-rc-text-primary">
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
  icon: typeof Compass;
  children: ReactNode;
}) {
  return (
    <section className="space-y-4 rounded-lg border border-rc-border-primary bg-rc-bg-surface px-5 py-5">
      <div className="flex items-start gap-3">
        <div className="rounded-md bg-rc-bg-secondary p-2 text-rc-text-primary">
          <Icon size={18} />
        </div>
        <div>
          <h3 className="text-base font-semibold text-rc-text-primary">{title}</h3>
          <p className="mt-1 text-sm text-rc-text-tertiary">{description}</p>
        </div>
      </div>
      {children}
    </section>
  );
}

export function CodexOperationsPanel() {
  const [threadsResponse, setThreadsResponse] = useState<CodexJsonValue>(null);
  const [operationResponse, setOperationResponse] = useState<CodexJsonValue>(null);
  const [selectedThreadId, setSelectedThreadId] = useState('');
  const [includeTurns, setIncludeTurns] = useState(true);
  const [searchTerm, setSearchTerm] = useState('');
  const [includeArchived, setIncludeArchived] = useState(false);

  const [threadName, setThreadName] = useState('');
  const [goalText, setGoalText] = useState('');
  const [turnId, setTurnId] = useState('');
  const [rollbackTurns, setRollbackTurns] = useState('1');
  const [steerMessage, setSteerMessage] = useState('');
  const [turnsLimit, setTurnsLimit] = useState('20');

  const [featureName, setFeatureName] = useState('');
  const [featureEnabled, setFeatureEnabled] = useState(true);

  const [cwdList, setCwdList] = useState('');
  const [forceReloadSkills, setForceReloadSkills] = useState(false);
  const [skillId, setSkillId] = useState('');
  const [skillEnabled, setSkillEnabled] = useState(true);
  const [pluginId, setPluginId] = useState('');
  const [pluginSource, setPluginSource] = useState('');
  const [marketplaceSource, setMarketplaceSource] = useState('');
  const [mcpServer, setMcpServer] = useState('');
  const [mcpResourceUri, setMcpResourceUri] = useState('');
  const [mcpTool, setMcpTool] = useState('');
  const [mcpArgs, setMcpArgs] = useState('{}');
  const [mcpStatusDetail, setMcpStatusDetail] = useState<'full' | 'toolsAndAuthOnly'>('full');
  const [mcpStatusLimit, setMcpStatusLimit] = useState('50');
  const [reviewPrompt, setReviewPrompt] = useState('');

  const [configIncludeLayers, setConfigIncludeLayers] = useState(true);
  const [configKey, setConfigKey] = useState('');
  const [configValue, setConfigValue] = useState('null');
  const [configMergeStrategy, setConfigMergeStrategy] = useState<'replace' | 'upsert'>('replace');
  const [configBatchEdits, setConfigBatchEdits] = useState(
    '[\n  {\n    "keyPath": "model",\n    "value": "gpt-5",\n    "mergeStrategy": "replace"\n  }\n]',
  );
  const [configReloadUserConfig, setConfigReloadUserConfig] = useState(true);

  const [memoryEnabled, setMemoryEnabled] = useState(true);
  const [feedbackClassification, setFeedbackClassification] = useState('bug');
  const [feedbackReason, setFeedbackReason] = useState('');
  const [feedbackIncludeLogs, setFeedbackIncludeLogs] = useState(true);

  const [appServerMethod, setAppServerMethod] = useState('model/list');
  const [appServerParams, setAppServerParams] = useState('{}');
  const [appServerResponse, setAppServerResponse] = useState<CodexJsonValue>(null);

  const [nativeParams, setNativeParams] = useState('{}');
  const [nativeShellCommand, setNativeShellCommand] = useState('');
  const [nativeInjectItems, setNativeInjectItems] = useState('[]');
  const [nativeFsPath, setNativeFsPath] = useState('');
  const [nativeFsTargetPath, setNativeFsTargetPath] = useState('');
  const [nativeFsContents, setNativeFsContents] = useState('');
  const [nativeFuzzyQuery, setNativeFuzzyQuery] = useState('');
  const [nativeRealtimeText, setNativeRealtimeText] = useState('');

  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const threads = useMemo(() => getThreads(threadsResponse), [threadsResponse]);

  async function run(label: string, fn: () => Promise<CodexJsonValue>) {
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

  function selectedThreadRequest() {
    const threadId = selectedThreadId.trim();
    if (!threadId) {
      setError('Select a Codex thread first.');
      return null;
    }
    return { sessionId: null, threadId };
  }

  function parseCwds(): string[] | null {
    const values = cwdList
      .split('\n')
      .map((value) => value.trim())
      .filter(Boolean);
    return values.length ? values : null;
  }

  function parseJsonInput(raw: string, label: string): { ok: true; value: unknown } | { ok: false } {
    try {
      return { ok: true, value: JSON.parse(raw.trim() || 'null') };
    } catch {
      setError(`${label} must be valid JSON.`);
      return { ok: false };
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
      setThreadName(nextThreads[0].name || '');
    }
  }

  async function operateThread(action: 'read' | 'resume' | 'fork' | 'archive' | 'unarchive') {
    if (!selectedThreadId) return;
    if (
      action === 'archive' &&
      !confirmRisk(`Archive Codex thread ${selectedThreadId}? It will be hidden from active thread lists.`)
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
    setOperationResponse(response);
  }

  async function setThreadNameTyped() {
    const base = selectedThreadRequest();
    const name = threadName.trim();
    if (!base || !name) return;
    const response = await run('thread-set-name', () => tauri.codexThreadSetName({ ...base, name }));
    setOperationResponse(response);
  }

  async function goalAction(action: 'set' | 'get' | 'clear') {
    const base = selectedThreadRequest();
    if (!base) return;
    if (action === 'clear' && !confirmRisk(`Clear goal for Codex thread ${base.threadId}?`)) return;
    const response = await run(`goal-${action}`, () => {
      if (action === 'set') return tauri.codexThreadGoalSet({ ...base, text: goalText });
      if (action === 'get') return tauri.codexThreadGoalGet(base);
      return tauri.codexThreadGoalClear(base);
    });
    setOperationResponse(response);
  }

  async function listTurns() {
    const base = selectedThreadRequest();
    if (!base) return;
    const limit = Number.parseInt(turnsLimit, 10);
    const response = await run('turns-list', () =>
      tauri.codexThreadTurnsList({ ...base, limit: Number.isFinite(limit) ? limit : null }),
    );
    setOperationResponse(response);
  }

  async function compactThread() {
    const base = selectedThreadRequest();
    if (!base) return;
    const response = await run('compact-start', () => tauri.codexThreadCompactStart(base));
    setOperationResponse(response);
  }

  async function rollbackThread() {
    const base = selectedThreadRequest();
    const numTurns = Number.parseInt(rollbackTurns, 10);
    if (!base || !Number.isFinite(numTurns) || numTurns < 1) return;
    if (!confirmRisk(`Rollback ${numTurns} turn(s) from Codex thread ${base.threadId}?`)) return;
    const response = await run('rollback', () => tauri.codexThreadRollback({ ...base, numTurns }));
    setOperationResponse(response);
  }

  async function steerTurn() {
    const base = selectedThreadRequest();
    const message = steerMessage.trim();
    const expectedTurnId = turnId.trim();
    if (!base || !message || !expectedTurnId) return;
    const response = await run('turn-steer', () =>
      tauri.codexTurnSteer({ ...base, expectedTurnId, message }),
    );
    setOperationResponse(response);
  }

  async function interruptTurn() {
    const base = selectedThreadRequest();
    if (!base) return;
    const response = await run('turn-interrupt', () =>
      tauri.codexTurnInterrupt({ ...base, turnId: turnId.trim() || null }),
    );
    setOperationResponse(response);
  }

  async function discovery(action: string) {
    const response = await run(action, () => {
      if (action === 'models') return tauri.codexModelList();
      if (action === 'account') return tauri.codexAccountRead();
      if (action === 'rate-limits') return tauri.codexAccountRateLimitsRead();
      if (action === 'collab') return tauri.codexCollaborationModeList();
      if (action === 'experimental') return tauri.codexExperimentalFeatureList();
      return tauri.codexExperimentalFeatureSet({ feature: featureName.trim(), enabled: featureEnabled });
    });
    setOperationResponse(response);
  }

  async function ecosystem(action: string) {
    const response = await run(action, () => {
      if (action === 'apps') return tauri.codexAppsList();
      if (action === 'skills') {
        return tauri.codexSkillsList({
          cwds: parseCwds(),
          forceReload: forceReloadSkills,
        });
      }
      if (action === 'skill-config') {
        return tauri.codexSkillsConfigWrite({ skillId: skillId.trim(), enabled: skillEnabled });
      }
      if (action === 'plugins') return tauri.codexPluginList({ cwds: parseCwds() });
      if (action === 'plugin-read') return tauri.codexPluginRead({ pluginId: pluginId.trim() });
      if (action === 'plugin-install') return tauri.codexPluginInstall({ source: pluginSource.trim() });
      if (action === 'plugin-uninstall') return tauri.codexPluginUninstall({ pluginId: pluginId.trim() });
      if (action === 'marketplace-add') return tauri.codexMarketplaceAdd({ source: marketplaceSource.trim() });
      if (action === 'marketplace-remove') return tauri.codexMarketplaceRemove({ source: marketplaceSource.trim() });
      return tauri.codexMarketplaceUpgrade({ source: marketplaceSource.trim() });
    });
    setOperationResponse(response);
  }

  async function guardedEcosystem(action: string, message: string) {
    if (!confirmRisk(message)) return;
    await ecosystem(action);
  }

  async function mcpAction(action: 'refresh' | 'status' | 'resource' | 'tool') {
    if (action === 'refresh') {
      const response = await run('mcp-refresh', async () => {
        await tauri.codexMcpRefresh(null);
        return tauri.codexMcpStatus({ sessionId: null, detail: mcpStatusDetail, limit: 50 });
      });
      setOperationResponse(response);
      return;
    }
    if (action === 'status') {
      const limit = Number.parseInt(mcpStatusLimit, 10);
      const response = await run('mcp-status', () =>
        tauri.codexMcpStatus({
          sessionId: null,
          detail: mcpStatusDetail,
          limit: Number.isFinite(limit) ? limit : null,
        }),
      );
      setOperationResponse(response);
      return;
    }
    const server = mcpServer.trim();
    if (!server) return;
    if (action === 'resource') {
      const uri = mcpResourceUri.trim();
      if (!uri) return;
      const response = await run('mcp-resource', () =>
        tauri.codexMcpReadResource({ sessionId: null, server, uri }),
      );
      setOperationResponse(response);
      return;
    }
    const base = selectedThreadRequest();
    const tool = mcpTool.trim();
    if (!base || !tool) return;
    const parsedArgs = parseJsonInput(mcpArgs, 'MCP tool arguments');
    if (!parsedArgs.ok) return;
    const response = await run('mcp-tool', () =>
      tauri.codexMcpCallTool({
        ...base,
        server,
        tool,
        arguments: parsedArgs.value,
      }),
    );
    setOperationResponse(response);
  }

  async function mcpOAuthLogin() {
    const server = mcpServer.trim();
    if (!server) return;
    const response = await run('mcp-oauth-login', () => tauri.codexMcpOAuthLogin({ sessionId: null, server }));
    setOperationResponse(response);
  }

  async function readConfig() {
    const response = await run('config-read', () => tauri.codexReadConfig(configIncludeLayers));
    setOperationResponse(response);
  }

  async function writeConfigValue() {
    const keyPath = configKey.trim();
    if (!keyPath) return;
    const parsedValue = parseJsonInput(configValue, 'Config value');
    if (!parsedValue.ok) return;
    if (!confirmRisk(`Write Codex config value "${keyPath}"?`)) return;
    const response = await run('config-value-write', () =>
      tauri.codexWriteConfigValue({
        keyPath,
        value: parsedValue.value,
        mergeStrategy: configMergeStrategy,
      }),
    );
    setOperationResponse(response);
  }

  async function writeConfigBatch() {
    const parsedEdits = parseJsonInput(configBatchEdits, 'Config batch edits');
    if (!parsedEdits.ok) return;
    if (!Array.isArray(parsedEdits.value)) {
      setError('Config batch edits must be a JSON array.');
      return;
    }
    if (!confirmRisk(`Write ${parsedEdits.value.length} Codex config edit(s)?`)) return;
    const response = await run('config-batch-write', () =>
      tauri.codexWriteConfigBatch({
        edits: parsedEdits.value as Parameters<typeof tauri.codexWriteConfigBatch>[0]['edits'],
        reloadUserConfig: configReloadUserConfig,
      }),
    );
    setOperationResponse(response);
  }

  async function setMemoryMode() {
    const base = selectedThreadRequest();
    if (!base) return;
    const response = await run('memory-mode', () =>
      tauri.codexSetThreadMemoryMode({ ...base, enabled: memoryEnabled }),
    );
    setOperationResponse(response);
  }

  async function resetMemories() {
    if (!confirmRisk('Reset all Codex memories? This affects future threads.')) return;
    const response = await run('memory-reset', () => tauri.codexResetMemories());
    setOperationResponse(response);
  }

  async function uploadFeedback() {
    const classification = feedbackClassification.trim();
    if (!classification) return;
    const response = await run('feedback-upload', () =>
      tauri.codexUploadFeedback({
        classification,
        reason: feedbackReason.trim() || null,
        threadId: selectedThreadId.trim() || null,
        includeLogs: feedbackIncludeLogs,
      }),
    );
    setOperationResponse(response);
  }

  async function startReview() {
    const base = selectedThreadRequest();
    if (!base) return;
    const response = await run('review-start', () =>
      tauri.codexReviewStart({ ...base, prompt: reviewPrompt.trim() || null }),
    );
    setOperationResponse(response);
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
        `Send raw Codex app-server request "${method}"? Some methods can mutate threads, config, files, or sandbox setup.`,
      )
    ) {
      return;
    }
    const response = await run('app-server-request', () =>
      tauri.codexAppServerRequest({ method, params: parsedParams }),
    );
    setAppServerResponse(response);
  }

  function parseNativeParams() {
    const parsed = parseJsonInput(nativeParams, 'Advanced native params');
    if (!parsed.ok) return null;
    if (parsed.value != null && (typeof parsed.value !== 'object' || Array.isArray(parsed.value))) {
      setError('Advanced native params must be a JSON object.');
      return null;
    }
    return (parsed.value ?? {}) as Record<string, unknown>;
  }

  function nativeThreadRequest() {
    return {
      sessionId: null,
      threadId: selectedThreadId.trim() || null,
      params: parseNativeParams(),
    };
  }

  async function nativeThreadAction(
    action:
      | 'thread-start'
      | 'loaded-list'
      | 'unsubscribe'
      | 'shell'
      | 'background-clean'
      | 'inject'
      | 'turn-start',
  ) {
    const base = nativeThreadRequest();
    if (!base.params) return;
    if ((action === 'unsubscribe' || action === 'inject' || action === 'turn-start') && !base.threadId) {
      setError('Select or enter a loaded Codex thread first.');
      return;
    }
    const response = await run(action, () => {
      if (action === 'thread-start') return tauri.codexThreadStart(base);
      if (action === 'loaded-list') return tauri.codexThreadLoadedList(base);
      if (action === 'unsubscribe') return tauri.codexThreadUnsubscribe(base);
      if (action === 'background-clean') {
        if (!confirmRisk('Clean Codex background terminals for this session?')) {
          return Promise.resolve(null);
        }
        return tauri.codexThreadBackgroundTerminalsClean(base);
      }
      if (action === 'shell') {
        const command = nativeShellCommand.trim();
        if (!command) return Promise.resolve(null);
        if (!confirmRisk(`Run unsandboxed Codex thread shell command "${command}"?`)) {
          return Promise.resolve(null);
        }
        return tauri.codexThreadShellCommand({ ...base, command });
      }
      if (action === 'inject') {
        const parsedItems = parseJsonInput(nativeInjectItems, 'Inject items');
        if (!parsedItems.ok) return Promise.resolve(null);
        if (!confirmRisk(`Inject items into Codex thread ${base.threadId}?`)) {
          return Promise.resolve(null);
        }
        return tauri.codexThreadInjectItems({ ...base, params: { ...base.params, items: parsedItems.value } });
      }
      return tauri.codexTurnStart({ ...base, prompt: nativeRealtimeText.trim() || null });
    });
    setOperationResponse(response);
  }

  async function nativeAccountAction(
    action:
      | 'login'
      | 'login-cancel'
      | 'logout'
      | 'config-requirements'
      | 'external-detect'
      | 'external-import'
      | 'windows-sandbox',
  ) {
    const params = parseNativeParams();
    if (!params) return;
    const response = await run(action, () => {
      if (action === 'login') return tauri.codexAccountLogin({ params });
      if (action === 'login-cancel') return tauri.codexAccountLoginCancel();
      if (action === 'logout') {
        if (!confirmRisk('Log out the active Codex account?')) return Promise.resolve(null);
        return tauri.codexAccountLogout();
      }
      if (action === 'config-requirements') return tauri.codexConfigRequirementsRead();
      if (action === 'external-detect') return tauri.codexExternalAgentConfigDetect();
      if (action === 'external-import') {
        if (!confirmRisk('Import detected external agent configuration into Codex?')) return Promise.resolve(null);
        return tauri.codexExternalAgentConfigImport({ params });
      }
      if (!confirmRisk('Start Windows sandbox setup? This may change local system configuration.')) {
        return Promise.resolve(null);
      }
      return tauri.codexWindowsSandboxSetupStart();
    });
    setOperationResponse(response);
  }

  async function nativeFsAction(
    action: 'read' | 'write' | 'mkdir' | 'metadata' | 'dir' | 'remove' | 'copy' | 'watch' | 'unwatch',
  ) {
    const path = nativeFsPath.trim();
    if (!path) return;
    const params = parseNativeParams();
    if (!params) return;
    const response = await run(`fs-${action}`, () => {
      if (action === 'read') return tauri.codexFsReadFile({ path, params });
      if (action === 'metadata') return tauri.codexFsGetMetadata({ path, params });
      if (action === 'dir') return tauri.codexFsReadDirectory({ path, params });
      if (action === 'watch') return tauri.codexFsWatch({ path, params });
      if (action === 'unwatch') return tauri.codexFsUnwatch({ path, params });
      if (action === 'write') {
        if (!confirmRisk(`Write file "${path}" through Codex FS API?`)) return Promise.resolve(null);
        return tauri.codexFsWriteFile({ path, contents: nativeFsContents, params });
      }
      if (action === 'mkdir') {
        if (!confirmRisk(`Create directory "${path}" through Codex FS API?`)) return Promise.resolve(null);
        return tauri.codexFsCreateDirectory({ path, params });
      }
      if (action === 'remove') {
        if (!confirmRisk(`Remove "${path}" through Codex FS API?`)) return Promise.resolve(null);
        return tauri.codexFsRemove({ path, params });
      }
      const to = nativeFsTargetPath.trim();
      if (!to) return Promise.resolve(null);
      if (!confirmRisk(`Copy "${path}" to "${to}" through Codex FS API?`)) return Promise.resolve(null);
      return tauri.codexFsCopy({ from: path, to, params });
    });
    setOperationResponse(response);
  }

  async function nativeRealtimeAction(action: 'voices' | 'start' | 'append' | 'stop') {
    const params = parseNativeParams();
    if (!params) return;
    const response = await run(`realtime-${action}`, () => {
      if (action === 'voices') return tauri.codexRealtimeVoicesList();
      if (action === 'start') return tauri.codexRealtimeStart({ params });
      if (action === 'append') return tauri.codexRealtimeAppendText({ text: nativeRealtimeText, params });
      return tauri.codexRealtimeStop();
    });
    setOperationResponse(response);
  }

  async function nativeFuzzyAction(action: 'search' | 'session-start' | 'session-update' | 'session-stop') {
    const query = nativeFuzzyQuery.trim();
    if (!query) return;
    const params = parseNativeParams();
    if (!params) return;
    const request = { query, cwd: nativeFsPath.trim() || null, params };
    const response = await run(`fuzzy-${action}`, () => {
      if (action === 'search') return tauri.codexFuzzyFileSearch(request);
      if (action === 'session-start') return tauri.codexFuzzyFileSearchSessionStart(request);
      if (action === 'session-update') return tauri.codexFuzzyFileSearchSessionUpdate(request);
      return tauri.codexFuzzyFileSearchSessionStop(request);
    });
    setOperationResponse(response);
  }

  return (
    <div className="space-y-6" data-testid="codex-operations">
      <div className="rounded-md border border-rc-border-focus bg-rc-accent-primary px-4 py-4 text-white shadow-sm">
        <div className="flex items-start gap-3">
          <div className="rounded-md bg-rc-bg-surface/10 p-2">
            <TerminalSquare size={20} />
          </div>
          <div>
            <h2 className="text-base font-semibold">Codex Native Control Surface</h2>
            <p className="mt-1 text-sm text-white/70">
              Typed front-end entries for official Codex app-server operations, with a guarded raw escape hatch for new or experimental methods.
            </p>
          </div>
        </div>
      </div>

      {error && (
        <div className="rounded-lg border border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-3 text-sm text-rc-accent-error">
          {error}
        </div>
      )}

      <Section
        title="Thread lifecycle"
        description="Rename, goals, turns, compact, rollback, steer, interrupt, plus existing read/resume/fork/archive controls."
        icon={GitFork}
      >
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_180px_140px]">
          <input
            value={searchTerm}
            onChange={(event) => setSearchTerm(event.target.value)}
            placeholder="Search thread preview/title"
            className="rounded-md border border-rc-border-primary px-3 py-2 text-sm"
          />
          <label className="flex items-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm text-rc-text-secondary">
            <input
              type="checkbox"
              checked={includeArchived}
              onChange={(event) => setIncludeArchived(event.target.checked)}
            />
            Include archived
          </label>
          <button
            type="button"
            onClick={() => void refreshThreads()}
            disabled={busy === 'threads'}
            className="inline-flex items-center justify-center gap-2 rounded-md bg-rc-accent-primary px-4 py-2 text-sm font-medium text-white disabled:opacity-60"
          >
            <RefreshCw size={14} className={busy === 'threads' ? 'animate-spin' : ''} />
            Refresh threads
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
                    setThreadName(thread.name || '');
                  }}
                  className={`w-full rounded-md border px-4 py-3 text-left transition-colors ${
                    selectedThreadId === thread.id
                      ? 'border-rc-border-focus bg-rc-bg-active'
                      : 'border-rc-border-primary bg-rc-bg-surface hover:bg-rc-bg-hover'
                  }`}
                >
                  <div className="truncate text-sm font-semibold text-rc-text-primary">
                    {thread.name || thread.preview || thread.id}
                  </div>
                  <div className="mt-1 truncate text-xs text-rc-text-tertiary">{thread.id}</div>
                  <div className="mt-1 text-xs text-rc-text-tertiary">
                    {thread.modelProvider ?? 'provider'} · {formatTime(thread.updatedAt)}
                  </div>
                </button>
              ))}
            </div>
            <div className="space-y-2">
              <label className="flex items-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm text-rc-text-secondary">
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
                  className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2 text-sm font-medium text-rc-text-primary hover:bg-rc-bg-hover disabled:opacity-60"
                >
                  {action}
                </button>
              ))}
            </div>
          </div>
        )}

        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_160px]">
          <input
            value={threadName}
            onChange={(event) => setThreadName(event.target.value)}
            placeholder="New thread name"
            className="rounded-md border border-rc-border-primary px-3 py-2 text-sm"
          />
          <button type="button" onClick={() => void setThreadNameTyped()} className="rounded-md bg-rc-accent-primary px-4 py-2 text-sm font-medium text-white">
            Set name
          </button>
        </div>

        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_110px_110px_110px]">
          <input
            value={goalText}
            onChange={(event) => setGoalText(event.target.value)}
            placeholder="Thread goal text"
            className="rounded-md border border-rc-border-primary px-3 py-2 text-sm"
          />
          <button type="button" onClick={() => void goalAction('set')} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary">Set goal</button>
          <button type="button" onClick={() => void goalAction('get')} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary">Get goal</button>
          <button type="button" onClick={() => void goalAction('clear')} className="rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-3 py-2 text-sm font-medium text-rc-accent-error">Clear goal</button>
        </div>

        <div className="grid gap-3 lg:grid-cols-[120px_160px_160px_140px_140px]">
          <input value={turnsLimit} onChange={(event) => setTurnsLimit(event.target.value)} placeholder="limit" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <input value={turnId} onChange={(event) => setTurnId(event.target.value)} placeholder="active turn id" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <input value={rollbackTurns} onChange={(event) => setRollbackTurns(event.target.value)} placeholder="rollback turns" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <button type="button" onClick={() => void listTurns()} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary">List turns</button>
          <button type="button" onClick={() => void compactThread()} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary">Compact</button>
        </div>

        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_130px_130px_130px]">
          <input value={steerMessage} onChange={(event) => setSteerMessage(event.target.value)} placeholder="Steer message" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <button type="button" onClick={() => void steerTurn()} className="inline-flex items-center justify-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary"><Send size={14} />Steer</button>
          <button type="button" onClick={() => void interruptTurn()} className="inline-flex items-center justify-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary"><Square size={14} />Interrupt</button>
          <button type="button" onClick={() => void rollbackThread()} className="inline-flex items-center justify-center gap-2 rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-3 py-2 text-sm font-medium text-rc-accent-error"><RotateCcw size={14} />Rollback</button>
        </div>
      </Section>

      <Section title="Discovery" description="Read-only discovery for models, account, rate limits, collaboration modes, and experimental flags." icon={Compass}>
        <div className="flex flex-wrap gap-3">
          <button type="button" onClick={() => void discovery('models')} className="rounded-md bg-rc-accent-primary px-4 py-2 text-sm font-medium text-white">List models</button>
          <button type="button" onClick={() => void discovery('account')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Read account</button>
          <button type="button" onClick={() => void discovery('rate-limits')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Rate limits</button>
          <button type="button" onClick={() => void discovery('collab')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Collab modes</button>
          <button type="button" onClick={() => void discovery('experimental')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Experimental flags</button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_180px_140px]">
          <input value={featureName} onChange={(event) => setFeatureName(event.target.value)} placeholder="experimental feature" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <label className="flex items-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm text-rc-text-secondary"><input type="checkbox" checked={featureEnabled} onChange={(event) => setFeatureEnabled(event.target.checked)} />enabled</label>
          <button type="button" onClick={() => void guardedEcosystem('noop', 'noop')} className="hidden">noop</button>
          <button type="button" onClick={() => void discovery('feature-set')} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary">Set feature</button>
        </div>
      </Section>

      <Section title="Codex ecosystem" description="Manage apps, skills, plugins, marketplace sources, MCP OAuth login, and review startup." icon={Boxes}>
        <textarea value={cwdList} onChange={(event) => setCwdList(event.target.value)} placeholder="Optional CWDs, one per line" className="min-h-20 w-full rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
        <div className="flex flex-wrap gap-3">
          <button type="button" onClick={() => void ecosystem('apps')} className="rounded-md bg-rc-accent-primary px-4 py-2 text-sm font-medium text-white">List apps</button>
          <button type="button" onClick={() => void ecosystem('skills')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">List skills</button>
          <button type="button" onClick={() => void ecosystem('plugins')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">List plugins</button>
          <label className="flex items-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm text-rc-text-secondary"><input type="checkbox" checked={forceReloadSkills} onChange={(event) => setForceReloadSkills(event.target.checked)} />force reload skills</label>
        </div>
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_180px_150px]">
          <input value={skillId} onChange={(event) => setSkillId(event.target.value)} placeholder="skill id" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <label className="flex items-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm text-rc-text-secondary"><input type="checkbox" checked={skillEnabled} onChange={(event) => setSkillEnabled(event.target.checked)} />skill enabled</label>
          <button type="button" onClick={() => void guardedEcosystem('skill-config', `Write skill config for ${skillId.trim()}?`)} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-3 py-2 text-sm font-medium text-rc-accent-warning">Write skill config</button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_130px_130px_130px]">
          <input value={pluginId} onChange={(event) => setPluginId(event.target.value)} placeholder="plugin id" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <input value={pluginSource} onChange={(event) => setPluginSource(event.target.value)} placeholder="plugin source" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <button type="button" onClick={() => void ecosystem('plugin-read')} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary">Read plugin</button>
          <button type="button" onClick={() => void guardedEcosystem('plugin-install', `Install plugin from ${pluginSource.trim()}?`)} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-3 py-2 text-sm font-medium text-rc-accent-warning">Install plugin</button>
          <button type="button" onClick={() => void guardedEcosystem('plugin-uninstall', `Uninstall plugin ${pluginId.trim()}?`)} className="rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-3 py-2 text-sm font-medium text-rc-accent-error">Uninstall plugin</button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_130px_150px_150px]">
          <input value={marketplaceSource} onChange={(event) => setMarketplaceSource(event.target.value)} placeholder="marketplace source" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <button type="button" onClick={() => void guardedEcosystem('marketplace-add', `Add marketplace source ${marketplaceSource.trim()}?`)} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-3 py-2 text-sm font-medium text-rc-accent-warning">Add marketplace</button>
          <button type="button" onClick={() => void guardedEcosystem('marketplace-remove', `Remove marketplace source ${marketplaceSource.trim()}?`)} className="rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-3 py-2 text-sm font-medium text-rc-accent-error">Remove marketplace</button>
          <button type="button" onClick={() => void guardedEcosystem('marketplace-upgrade', `Upgrade marketplace source ${marketplaceSource.trim()}?`)} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-3 py-2 text-sm font-medium text-rc-accent-warning">Upgrade marketplace</button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_150px_minmax(0,1fr)_140px]">
          <input value={mcpServer} onChange={(event) => setMcpServer(event.target.value)} placeholder="MCP OAuth server" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <button type="button" onClick={() => void mcpOAuthLogin()} className="inline-flex items-center justify-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary"><LogIn size={14} />MCP OAuth</button>
          <input value={reviewPrompt} onChange={(event) => setReviewPrompt(event.target.value)} placeholder="review prompt optional" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <button type="button" onClick={() => void startReview()} className="inline-flex items-center justify-center gap-2 rounded-md bg-rc-accent-primary px-3 py-2 text-sm font-medium text-white"><Flag size={14} />Start review</button>
        </div>
      </Section>

      <Section
        title="MCP tools"
        description="Refresh official MCP server config, inspect server status, read resources, and call tools against the selected thread."
        icon={PlugZap}
      >
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_180px_120px_140px_140px]">
          <input value={mcpServer} onChange={(event) => setMcpServer(event.target.value)} placeholder="MCP server" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <select value={mcpStatusDetail} onChange={(event) => setMcpStatusDetail(event.target.value as 'full' | 'toolsAndAuthOnly')} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm">
            <option value="full">full status</option>
            <option value="toolsAndAuthOnly">tools/auth only</option>
          </select>
          <input value={mcpStatusLimit} onChange={(event) => setMcpStatusLimit(event.target.value)} placeholder="status limit" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <button type="button" onClick={() => void mcpAction('refresh')} className="inline-flex items-center justify-center gap-2 rounded-md bg-rc-accent-primary px-3 py-2 text-sm font-medium text-white"><RefreshCw size={14} />MCP refresh</button>
          <button type="button" onClick={() => void mcpAction('status')} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary">MCP status</button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_160px]">
          <input value={mcpResourceUri} onChange={(event) => setMcpResourceUri(event.target.value)} placeholder="MCP resource URI" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <button type="button" onClick={() => void mcpAction('resource')} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary">Read resource</button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[180px_minmax(0,1fr)_minmax(0,1fr)_140px]">
          <input value={mcpTool} onChange={(event) => setMcpTool(event.target.value)} placeholder="MCP tool" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <input value={mcpArgs} onChange={(event) => setMcpArgs(event.target.value)} placeholder='{"key":"value"}' className="rounded-md border border-rc-border-primary px-3 py-2 font-mono text-xs" />
          <div className="rounded-md border border-rc-border-primary px-3 py-2 text-xs text-rc-text-tertiary">Tool calls use the selected thread id.</div>
          <button type="button" onClick={() => void mcpAction('tool')} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary">Call tool</button>
        </div>
      </Section>

      <Section
        title="Codex config"
        description="Read effective Codex config and write values or batch edits through the typed config bridge."
        icon={Database}
      >
        <div className="flex flex-wrap gap-3">
          <button type="button" onClick={() => void readConfig()} className="rounded-md bg-rc-accent-primary px-4 py-2 text-sm font-medium text-white">Read config</button>
          <label className="flex items-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm text-rc-text-secondary"><input type="checkbox" checked={configIncludeLayers} onChange={(event) => setConfigIncludeLayers(event.target.checked)} />include layers</label>
        </div>
        <div className="grid gap-3 lg:grid-cols-[220px_minmax(0,1fr)_160px_140px]">
          <input value={configKey} onChange={(event) => setConfigKey(event.target.value)} placeholder="config key path" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <input value={configValue} onChange={(event) => setConfigValue(event.target.value)} placeholder='"value", true, 123, or JSON' className="rounded-md border border-rc-border-primary px-3 py-2 font-mono text-xs" />
          <select value={configMergeStrategy} onChange={(event) => setConfigMergeStrategy(event.target.value as 'replace' | 'upsert')} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm">
            <option value="replace">replace</option>
            <option value="upsert">upsert</option>
          </select>
          <button type="button" onClick={() => void writeConfigValue()} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-3 py-2 text-sm font-medium text-rc-accent-warning">Write value</button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_180px_150px]">
          <textarea value={configBatchEdits} onChange={(event) => setConfigBatchEdits(event.target.value)} placeholder='[{"keyPath":"model","value":"gpt-5"}]' className="min-h-28 rounded-md border border-rc-border-primary px-3 py-2 font-mono text-xs" />
          <label className="flex items-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm text-rc-text-secondary"><input type="checkbox" checked={configReloadUserConfig} onChange={(event) => setConfigReloadUserConfig(event.target.checked)} />reload user config</label>
          <button type="button" onClick={() => void writeConfigBatch()} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-3 py-2 text-sm font-medium text-rc-accent-warning">Write batch</button>
        </div>
      </Section>

      <Section
        title="Memory and feedback"
        description="Set thread memory mode, reset global Codex memories, and upload Codex feedback."
        icon={MemoryStick}
      >
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_180px_150px]">
          <div className="rounded-md border border-rc-border-primary px-3 py-2 text-xs text-rc-text-tertiary">
            Memory mode applies to selected thread: {selectedThreadId || 'none selected'}
          </div>
          <label className="flex items-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm text-rc-text-secondary"><input type="checkbox" checked={memoryEnabled} onChange={(event) => setMemoryEnabled(event.target.checked)} />memory enabled</label>
          <button type="button" onClick={() => void setMemoryMode()} className="rounded-md border border-rc-border-primary px-3 py-2 text-sm font-medium text-rc-text-primary">Set memory mode</button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[160px_minmax(0,1fr)_180px_150px_150px]">
          <input value={feedbackClassification} onChange={(event) => setFeedbackClassification(event.target.value)} placeholder="classification" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <input value={feedbackReason} onChange={(event) => setFeedbackReason(event.target.value)} placeholder="feedback reason" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          <label className="flex items-center gap-2 rounded-md border border-rc-border-primary px-3 py-2 text-sm text-rc-text-secondary"><input type="checkbox" checked={feedbackIncludeLogs} onChange={(event) => setFeedbackIncludeLogs(event.target.checked)} />include logs</label>
          <button type="button" onClick={() => void uploadFeedback()} className="rounded-md bg-rc-accent-primary px-3 py-2 text-sm font-medium text-white">Upload feedback</button>
          <button type="button" onClick={() => void resetMemories()} className="rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-3 py-2 text-sm font-medium text-rc-accent-error">Reset memories</button>
        </div>
      </Section>

      <Section
        title="Advanced Native"
        description="Typed wrappers for newly exposed Codex native commands. Complex official params can be supplied as JSON."
        icon={TerminalSquare}
      >
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
          <textarea
            value={nativeParams}
            onChange={(event) => setNativeParams(event.target.value)}
            placeholder='Advanced params JSON, for example {"cwd":"C:\\repo"}'
            className="min-h-24 rounded-md border border-rc-border-primary px-3 py-2 font-mono text-xs"
          />
          <div className="rounded-md border border-rc-border-primary px-3 py-2 text-xs text-rc-text-tertiary">
            Selected thread: {selectedThreadId || 'none'}. Destructive filesystem, shell, logout, import, sandbox, and cleanup actions require confirmation.
          </div>
        </div>

        <div className="space-y-3 rounded-md border border-rc-border-primary bg-rc-bg-secondary p-4">
          <div className="text-sm font-semibold text-rc-text-primary">Threads and turns</div>
          <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
            <input
              value={nativeShellCommand}
              onChange={(event) => setNativeShellCommand(event.target.value)}
              placeholder="Thread shell command"
              className="rounded-md border border-rc-border-primary px-3 py-2 text-sm"
            />
            <textarea
              value={nativeInjectItems}
              onChange={(event) => setNativeInjectItems(event.target.value)}
              placeholder='Inject items JSON, for example [{"type":"message","text":"hi"}]'
              className="min-h-10 rounded-md border border-rc-border-primary px-3 py-2 font-mono text-xs"
            />
          </div>
          <div className="flex flex-wrap gap-3">
            <button type="button" onClick={() => void nativeThreadAction('thread-start')} className="rounded-md bg-rc-accent-primary px-4 py-2 text-sm font-medium text-white">Native thread start</button>
            <button type="button" onClick={() => void nativeThreadAction('loaded-list')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Loaded threads</button>
            <button type="button" onClick={() => void nativeThreadAction('unsubscribe')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Unsubscribe</button>
            <button type="button" onClick={() => void nativeThreadAction('turn-start')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Turn start</button>
            <button type="button" onClick={() => void nativeThreadAction('shell')} className="rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-2 text-sm font-medium text-rc-accent-error">Thread shell</button>
            <button type="button" onClick={() => void nativeThreadAction('background-clean')} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-4 py-2 text-sm font-medium text-rc-accent-warning">Clean terminals</button>
            <button type="button" onClick={() => void nativeThreadAction('inject')} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-4 py-2 text-sm font-medium text-rc-accent-warning">Inject items</button>
          </div>
        </div>

        <div className="space-y-3 rounded-md border border-rc-border-primary bg-rc-bg-surface p-4">
          <div className="text-sm font-semibold text-rc-text-primary">Account, config, and setup</div>
          <div className="flex flex-wrap gap-3">
            <button type="button" onClick={() => void nativeAccountAction('login')} className="rounded-md bg-rc-accent-primary px-4 py-2 text-sm font-medium text-white">Account login</button>
            <button type="button" onClick={() => void nativeAccountAction('login-cancel')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Cancel login</button>
            <button type="button" onClick={() => void nativeAccountAction('logout')} className="rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-2 text-sm font-medium text-rc-accent-error">Account logout</button>
            <button type="button" onClick={() => void nativeAccountAction('config-requirements')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Config requirements</button>
            <button type="button" onClick={() => void nativeAccountAction('external-detect')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">External detect</button>
            <button type="button" onClick={() => void nativeAccountAction('external-import')} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-4 py-2 text-sm font-medium text-rc-accent-warning">External import</button>
            <button type="button" onClick={() => void nativeAccountAction('windows-sandbox')} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-4 py-2 text-sm font-medium text-rc-accent-warning">Windows sandbox</button>
          </div>
        </div>

        <div className="space-y-3 rounded-md border border-rc-border-primary bg-rc-bg-secondary p-4">
          <div className="text-sm font-semibold text-rc-text-primary">Filesystem and fuzzy search</div>
          <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
            <input value={nativeFsPath} onChange={(event) => setNativeFsPath(event.target.value)} placeholder="FS path or fuzzy cwd" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
            <input value={nativeFsTargetPath} onChange={(event) => setNativeFsTargetPath(event.target.value)} placeholder="Copy target path" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
            <input value={nativeFuzzyQuery} onChange={(event) => setNativeFuzzyQuery(event.target.value)} placeholder="Fuzzy file query" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
            <textarea value={nativeFsContents} onChange={(event) => setNativeFsContents(event.target.value)} placeholder="Write file contents" className="min-h-10 rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
          </div>
          <div className="flex flex-wrap gap-3">
            <button type="button" onClick={() => void nativeFsAction('read')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">FS read</button>
            <button type="button" onClick={() => void nativeFsAction('dir')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">FS dir</button>
            <button type="button" onClick={() => void nativeFsAction('metadata')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">FS metadata</button>
            <button type="button" onClick={() => void nativeFsAction('watch')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">FS watch</button>
            <button type="button" onClick={() => void nativeFsAction('unwatch')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">FS unwatch</button>
            <button type="button" onClick={() => void nativeFsAction('write')} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-4 py-2 text-sm font-medium text-rc-accent-warning">FS write</button>
            <button type="button" onClick={() => void nativeFsAction('mkdir')} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-4 py-2 text-sm font-medium text-rc-accent-warning">FS mkdir</button>
            <button type="button" onClick={() => void nativeFsAction('copy')} className="rounded-md border border-rc-accent-warning-border bg-rc-accent-warning-bg px-4 py-2 text-sm font-medium text-rc-accent-warning">FS copy</button>
            <button type="button" onClick={() => void nativeFsAction('remove')} className="rounded-md border border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-2 text-sm font-medium text-rc-accent-error">FS remove</button>
            <button type="button" onClick={() => void nativeFuzzyAction('search')} className="rounded-md bg-rc-accent-primary px-4 py-2 text-sm font-medium text-white">Fuzzy search</button>
            <button type="button" onClick={() => void nativeFuzzyAction('session-start')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Fuzzy session start</button>
            <button type="button" onClick={() => void nativeFuzzyAction('session-update')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Fuzzy update</button>
            <button type="button" onClick={() => void nativeFuzzyAction('session-stop')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Fuzzy stop</button>
          </div>
        </div>

        <div className="space-y-3 rounded-md border border-rc-border-primary bg-rc-bg-surface p-4">
          <div className="text-sm font-semibold text-rc-text-primary">Realtime</div>
          <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_460px]">
            <input value={nativeRealtimeText} onChange={(event) => setNativeRealtimeText(event.target.value)} placeholder="Realtime text or turn prompt" className="rounded-md border border-rc-border-primary px-3 py-2 text-sm" />
            <div className="flex flex-wrap gap-3">
              <button type="button" onClick={() => void nativeRealtimeAction('voices')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Realtime voices</button>
              <button type="button" onClick={() => void nativeRealtimeAction('start')} className="rounded-md bg-rc-accent-primary px-4 py-2 text-sm font-medium text-white">Realtime start</button>
              <button type="button" onClick={() => void nativeRealtimeAction('append')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Append text</button>
              <button type="button" onClick={() => void nativeRealtimeAction('stop')} className="rounded-md border border-rc-border-primary px-4 py-2 text-sm font-medium text-rc-text-primary">Realtime stop</button>
            </div>
          </div>
        </div>
      </Section>

      <Section
        title="Raw app-server escape hatch"
        description="Guarded passthrough for app-server methods that do not have typed UI yet. Prefer the typed buttons above when available."
        icon={TerminalSquare}
      >
        <div className="grid gap-3 lg:grid-cols-[260px_minmax(0,1fr)_150px]">
          <input value={appServerMethod} onChange={(event) => setAppServerMethod(event.target.value)} placeholder="model/list" className="rounded-md border border-rc-border-primary px-3 py-2 font-mono text-xs" />
          <input value={appServerParams} onChange={(event) => setAppServerParams(event.target.value)} placeholder='{"threadId":"..."}' className="rounded-md border border-rc-border-primary px-3 py-2 font-mono text-xs" />
          <button type="button" onClick={() => void runAppServerRequest()} className="inline-flex items-center justify-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2 text-sm font-medium text-rc-text-primary"><ShieldAlert size={14} />Raw request</button>
        </div>
        <JsonBlock value={appServerResponse} />
      </Section>

      <JsonBlock value={operationResponse ?? threadsResponse} />
    </div>
  );
}
