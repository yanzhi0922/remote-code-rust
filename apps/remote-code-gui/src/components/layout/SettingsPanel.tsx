import {
  Archive,
  ArchiveRestore,
  Blocks,
  Bot,
  Brain,
  Check,
  ChevronDown,
  Copy,
  Eye,
  EyeOff,
  Gauge,
  GitBranch,
  Pencil,
  Plug,
  Plus,
  RefreshCw,
  SlidersHorizontal,
  TerminalSquare,
  Trash2,
  Wifi,
  X,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type {
  ClaudeModelMapping,
  FullSettings,
  ProbeModelResult,
  ProviderConfig,
  ProviderModel,
  SessionSummary,
} from '../../lib/types';
import { recordFrontendLog } from '../../lib/tauri';
import { useAppStore } from '../../stores/useAppStore';
import { ClaudeSettings } from '../settings/ClaudeSettings';
import { CodexSettings } from '../settings/CodexSettings';
import { RemoteTab } from '../settings/RemoteTab';
import { RooSettings } from '../settings/RooSettings';
import { McpTab } from './McpTab';
import { OperationsTab } from './OperationsTab';

interface SettingsPanelProps {
  open: boolean;
  onClose: () => void;
  initialTab?: SettingsTab;
}

export type SettingsTab = 'provider' | 'runtime' | 'claude' | 'roo' | 'codex' | 'mcp' | 'remote' | 'operations' | 'archive';

type TFn = (key: string, options?: Record<string, unknown>) => string;

function settingsTabs(t: TFn): Array<{ key: SettingsTab; label: string; icon: React.ElementType }> {
  return [
    { key: 'provider', label: 'Provider', icon: SlidersHorizontal },
    { key: 'runtime', label: t('settings.runtimeParams'), icon: Gauge },
    { key: 'claude', label: 'Claude', icon: Brain },
    { key: 'roo', label: 'Roo', icon: GitBranch },
    { key: 'codex', label: 'Codex', icon: Bot },
    { key: 'mcp', label: 'MCP', icon: Blocks },
    { key: 'remote', label: t('settings.remote'), icon: Wifi },
    { key: 'operations', label: t('settings.operations'), icon: TerminalSquare },
    { key: 'archive', label: t('settings.archive'), icon: Archive },
  ];
}

function protocols(t: TFn) {
  return [
    { value: 'openai', label: t('settings.openAiCompatible') },
    { value: 'anthropic', label: 'Anthropic Messages' },
    { value: 'bedrock', label: 'AWS Bedrock' },
    { value: 'vertex', label: 'Google Vertex' },
  ];
}

function emptyProviderConfig(): ProviderConfig {
  return {
    name: '',
    protocol: 'openai',
    base_url: '',
    api_key: '',
    model: '',
  };
}

function Field({
  label,
  children,
  hint,
}: {
  label: string;
  children: React.ReactNode;
  hint?: string;
}) {
  return (
    <div className="space-y-1.5">
      <label className="block text-sm font-medium text-rc-text-primary">{label}</label>
      {children}
      {hint && <p className="text-xs leading-5 text-rc-text-tertiary">{hint}</p>}
    </div>
  );
}

function RuntimePathRow({ label, path, t }: { label: string; path: string; t: TFn }) {
  const copyPath = () => {
    if (typeof navigator === 'undefined' || !navigator.clipboard) return;
    void navigator.clipboard.writeText(path);
  };

  return (
    <div className="grid gap-2 border-b border-rc-border-secondary px-3 py-2.5 last:border-b-0 sm:grid-cols-[180px_minmax(0,1fr)_auto] sm:items-center">
      <div className="text-xs font-medium uppercase tracking-wide text-rc-text-tertiary">
        {label}
      </div>
      <code className="min-w-0 break-all rounded bg-rc-bg-base px-2 py-1 font-mono text-xs text-rc-text-secondary">
        {path}
      </code>
      <button
        type="button"
        onClick={copyPath}
        className="inline-flex h-7 w-fit items-center gap-1 rounded-md border border-rc-border-primary px-2 text-xs font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
      >
        <Copy size={13} />
        {t('common.copy')}
      </button>
    </div>
  );
}

export function SettingsPanel({ open, onClose, initialTab = 'provider' }: SettingsPanelProps) {
  const { t } = useTranslation();
  const settings = useAppStore((state) => state.settings);
  const loadSettings = useAppStore((state) => state.loadSettings);
  const loadProviderConfigs = useAppStore((state) => state.loadProviderConfigs);
  const loadArchivedSessions = useAppStore((state) => state.loadArchivedSessions);
  const updateSettings = useAppStore((state) => state.updateSettings);

  const [activeTab, setActiveTab] = useState<SettingsTab>(initialTab);
  const [draft, setDraft] = useState<Partial<FullSettings>>({});
  const [saving, setSaving] = useState(false);
  const [settingsSearch, setSettingsSearch] = useState('');

  useEffect(() => {
    if (!open) return;
    void Promise.all([loadSettings(), loadProviderConfigs(), loadArchivedSessions()]);
    setDraft({});
    setActiveTab(initialTab);
  }, [initialTab, loadArchivedSessions, loadProviderConfigs, loadSettings, open]);

  if (!open) return null;

  const current = { ...settings, ...draft } as FullSettings;

  const applyDraft = (key: keyof FullSettings, value: unknown) => {
    setDraft((state) => ({ ...state, [key]: value }));
  };

  const handleSave = async () => {
    if (Object.keys(draft).length === 0) return;
    setSaving(true);
    try {
      await updateSettings(draft as Record<string, unknown>);
      setDraft({});
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-rc-bg-base">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
        className="grid h-full w-full grid-cols-[300px_minmax(0,1fr)] overflow-hidden bg-rc-bg-base text-rc-text-primary"
      >
        <aside className="flex min-h-0 flex-col border-r border-rc-border-secondary bg-rc-bg-sidebar">
          <div className="border-b border-rc-border-secondary px-3 py-3">
            <button
              onClick={onClose}
              aria-label={t('settings.backToApp')}
              className="inline-flex h-8 items-center gap-2 rounded-md px-2 text-xs font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
            >
              <X size={15} />
              {t('settings.backToApp')}
            </button>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto p-2">
            <div className="relative mb-2">
              <input
                value={settingsSearch}
                onChange={(e) => setSettingsSearch(e.target.value)}
                placeholder={t('settings.searchPlaceholder')}
                className="h-8 w-full rounded-md border border-rc-border-primary bg-rc-bg-base px-2.5 text-xs text-rc-text-primary outline-none placeholder:text-rc-text-tertiary focus:border-rc-border-focus"
              />
              {settingsSearch && (
                <button
                  type="button"
                  onClick={() => setSettingsSearch('')}
                  className="absolute right-2 top-1/2 -translate-y-1/2 text-rc-text-tertiary hover:text-rc-text-primary"
                >
                  <X size={12} />
                </button>
              )}
            </div>
            <div role="tablist" aria-label="Settings sections" className="space-y-0.5">
              {settingsTabs(t)
                .filter((tab) => !settingsSearch || tab.label.toLowerCase().includes(settingsSearch.toLowerCase()))
                .map((tab) => {
                const Icon = tab.icon;
                const selected = activeTab === tab.key;
                return (
                  <button
                    key={tab.key}
                    id={`settings-tab-${tab.key}`}
                    role="tab"
                    aria-selected={selected}
                    aria-controls={`settings-panel-${tab.key}`}
                    onClick={() => setActiveTab(tab.key)}
                    className={`flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm font-medium transition-colors ${
                      selected
                        ? 'bg-rc-bg-active text-rc-text-primary shadow-xs'
                        : 'text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary'
                    }`}
                  >
                    <Icon size={15} className="shrink-0" />
                    {tab.label}
                  </button>
                );
              })}
            </div>
          </div>
        </aside>

        <div className="flex min-h-0 flex-col rounded-tl-xl border-l border-t border-rc-border-secondary bg-rc-bg-chat shadow-xs">
          <header className="flex h-14 shrink-0 items-center justify-between border-b border-rc-border-secondary bg-rc-bg-surface px-6">
            <div className="min-w-0">
              <h2 className="truncate text-sm font-semibold text-rc-text-primary">
                {settingsTabs(t).find((tab) => tab.key === activeTab)?.label ?? t('settings.title')}
              </h2>
            </div>
            <button
              onClick={onClose}
              aria-label={t('settings.closeSettings')}
              className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
            >
              <X size={18} />
            </button>
          </header>

          <div
            id={`settings-panel-${activeTab}`}
            role="tabpanel"
            aria-labelledby={`settings-tab-${activeTab}`}
            className="min-h-0 flex-1 overflow-y-auto px-6 py-12"
          >
            <div className="mx-auto w-full max-w-[860px]">
              {!settings ? (
                <div className="py-10 text-sm text-rc-text-secondary">{t('settings.loadingSettings')}</div>
              ) : activeTab === 'provider' ? (
                <ProviderTab />
              ) : activeTab === 'mcp' ? (
                <McpTab />
              ) : activeTab === 'claude' ? (
                <ClaudeSettings
                  settings={current}
                  onUpdate={(updates) => setDraft((state) => ({ ...state, ...updates }))}
                />
              ) : activeTab === 'roo' ? (
                <RooSettings
                  settings={current}
                  onUpdate={(updates) => setDraft((state) => ({ ...state, ...updates }))}
                />
              ) : activeTab === 'codex' ? (
                <CodexSettings
                  settings={current}
                  onUpdate={(updates) => setDraft((state) => ({ ...state, ...updates }))}
                />
              ) : activeTab === 'remote' ? (
                <RemoteTab />
              ) : activeTab === 'operations' ? (
                <OperationsTab />
              ) : activeTab === 'archive' ? (
                <ArchiveTab />
              ) : (
                <RuntimeTab current={current} onChange={applyDraft} />
              )}
            </div>
          </div>

          <footer className="flex shrink-0 items-center justify-end gap-3 border-t border-rc-border-secondary bg-rc-bg-surface px-6 py-3">
            <button
              onClick={() => setDraft({})}
              className="rounded-md px-4 py-2 text-sm font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
            >
              {t('settings.resetUnsaved')}
            </button>
            <button
              onClick={() => {
                void handleSave();
              }}
              disabled={Object.keys(draft).length === 0 || saving}
              className="rounded-md bg-rc-accent-primary px-5 py-2 text-sm font-medium text-white transition-colors hover:bg-rc-accent-primary-hover disabled:cursor-not-allowed disabled:bg-rc-text-tertiary"
            >
              {saving ? t('settings.saving') : t('settings.saveButton')}
            </button>
          </footer>
        </div>
      </div>
    </div>
  );
}

function formatRelativeTime(iso: string, t: TFn): string {
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

function ArchiveRow({
  session,
  privacyMode,
  onRestore,
  t,
}: {
  session: SessionSummary;
  privacyMode: boolean;
  onRestore: () => void;
  t: TFn;
}) {
  return (
    <div className="flex items-center gap-3 rounded-md border border-rc-border-secondary bg-rc-bg-secondary px-3 py-2.5">
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-semibold text-rc-text-primary">
          {privacyMode ? t('settings.sessionArchived') : session.title}
        </div>
        <div className="mt-1 truncate text-xs text-rc-text-secondary">
          {privacyMode ? t('settings.pathHidden') : session.cwd}
        </div>
        <div className="mt-1 text-[11px] text-rc-text-tertiary">
          {session.provider_name}
          {session.model ? ` · ${session.model}` : ''} · {formatRelativeTime(session.updated_at, t)}
        </div>
      </div>

      <button
        onClick={onRestore}
        className="inline-flex items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2 text-sm font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
      >
        <ArchiveRestore size={14} />
        {t('common.restore')}
      </button>
    </div>
  );
}

function ArchiveTab() {
  const { t } = useTranslation();
  const archivedSessions = useAppStore((state) => state.archivedSessions);
  const restoreSession = useAppStore((state) => state.restoreSession);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);

  return (
    <div className="space-y-5">
      <div>
        <h3 className="text-sm font-semibold text-rc-text-primary">{t('settings.archiveHeading')}</h3>
      </div>

      {archivedSessions.length > 0 ? (
        <div className="space-y-2">
          {archivedSessions.map((session) => (
            <ArchiveRow
              key={session.id}
              session={session}
              privacyMode={privacyMode}
              t={t}
              onRestore={() => {
                void restoreSession(session.id);
              }}
            />
          ))}
        </div>
      ) : (
        <div className="rounded-md border border-dashed border-rc-border-primary px-3 py-5 text-sm text-rc-text-secondary">
          {t('settings.noArchivedSessions')}
        </div>
      )}
    </div>
  );
}

function ProviderTab() {
  const { t } = useTranslation();
  const providerConfigs = useAppStore((state) => state.providerConfigs);
  const saveProviderConfig = useAppStore((state) => state.saveProviderConfig);
  const deleteProviderConfig = useAppStore((state) => state.deleteProviderConfig);
  const setActiveProvider = useAppStore((state) => state.setActiveProvider);
  const setProviderEnabled = useAppStore((state) => state.setProviderEnabled);
  const setClaudeModelMapping = useAppStore((state) => state.setClaudeModelMapping);
  const addProviderModel = useAppStore((state) => state.addProviderModel);
  const updateProviderModel = useAppStore((state) => state.updateProviderModel);
  const removeProviderModel = useAppStore((state) => state.removeProviderModel);
  const probeProviderModel = useAppStore((state) => state.probeProviderModel);
  const refreshProviders = useAppStore((state) => state.refreshProviders);

  const providers = providerConfigs?.providers ?? [];
  const activeProviderName = providerConfigs?.active_provider ?? null;
  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [editingNew, setEditingNew] = useState(false);
  const [form, setForm] = useState<ProviderConfig>(emptyProviderConfig());
  const [showApiKey, setShowApiKey] = useState(false);
  // Most recent probe failure (inline error banner).  Cleared on the
  // next successful probe, when the user dismisses it, or when the
  // active tab changes.
  const [probeError, setProbeError] = useState<string | null>(null);
  // Tracks the (message, timestamp) of the last banner we set so we
  // can coalesce identical 1-s rate-limit messages instead of stacking
  // 5 banners when a user spam-clicks the Plug icon.
  const lastProbeErrorRef = useRef<{ message: string; at: number } | null>(null);

  // Default selection: prefer active provider, else first row.
  useEffect(() => {
    if (selectedName && providers.some((p) => p.name === selectedName)) return;
    const next = activeProviderName && providers.some((p) => p.name === activeProviderName)
      ? activeProviderName
      : providers[0]?.name ?? null;
    setSelectedName(next);
  }, [activeProviderName, providers, selectedName]);

  const selected = useMemo(
    () => providers.find((p) => p.name === selectedName) ?? null,
    [providers, selectedName],
  );

  // --- List helpers ---
  // All providers (Builtins and user-added Customs) appear in a single
  // flat list under one header. Builtins are protected from deletion by the
  // backend, but visually they are peers of every other provider.
  const allProviders = providers;

  // --- Add new provider ---
  const startAddNew = () => {
    setEditingNew(true);
    setSelectedName(null);
    setForm({ ...emptyProviderConfig(), group: 'custom' });
    setShowApiKey(false);
  };

  // --- Edit existing provider ---
  const startEdit = (config: ProviderConfig) => {
    setEditingNew(false);
    setForm({
      ...config,
      base_url: config.base_url ?? '',
      anthropic_base_url: config.anthropic_base_url ?? '',
      openai_base_url: config.openai_base_url ?? '',
      api_key: '',
      model: config.model ?? '',
      models: config.models ? config.models.map((m) => ({ ...m })) : [],
      claude_model_mapping: { ...(config.claude_model_mapping ?? {}) },
      profiles: config.profiles ? config.profiles.map((p) => ({ ...p })) : [],
      api_key_stored: config.api_key_stored ?? false,
    });
    setShowApiKey(false);
  };

  const cancelEdit = () => {
    setEditingNew(false);
    setForm(emptyProviderConfig());
    setShowApiKey(false);
  };

  const handleSave = async () => {
    if (!form.name.trim()) return;
    const trimmedName = form.name.trim();
    const existingNames = new Set(
      providers.filter((p) => p.name !== selectedName).map((p) => p.name),
    );
    if (existingNames.has(trimmedName)) {
      // eslint-disable-next-line no-alert
      const proceed = window.confirm(
        t('settings.providerExists', { name: trimmedName }),
      );
      if (!proceed) return;
    }
    const shouldActivate = editingNew || activeProviderName === trimmedName;
    await saveProviderConfig(
      {
        name: trimmedName,
        protocol: form.protocol,
        base_url: form.base_url?.trim() || undefined,
        anthropic_base_url: form.anthropic_base_url?.trim() || undefined,
        openai_base_url: form.openai_base_url?.trim() || undefined,
        api_key: form.api_key?.trim() || undefined,
        model: form.model?.trim() || undefined,
        models: form.models?.filter((m) => m.id.trim()),
        claude_model_mapping: form.claude_model_mapping,
        group: form.group,
        enabled: form.enabled,
      },
      shouldActivate,
    );
    cancelEdit();
    setSelectedName(trimmedName);
  };

  // --- Delete (with built-in protection) ---
  const handleDelete = async (name: string) => {
    const target = providers.find((p) => p.name === name);
    if (target?.group === 'builtin') {
      // eslint-disable-next-line no-alert
      window.alert(t('settings.builtinCannotDelete'));
      return;
    }
    await deleteProviderConfig(name);
    if (selectedName === name) setSelectedName(null);
  };

  // --- Model catalog CRUD ---
  const handleAddModel = async (model: ProviderModel) => {
    if (!selected) return;
    const trimmed = model.id.trim();
    if (!trimmed) return;
    await addProviderModel(selected.name, { id: trimmed, display_name: model.display_name?.trim() || undefined });
  };

  const handleUpdateModel = async (oldId: string, model: ProviderModel) => {
    if (!selected) return;
    const trimmed = model.id.trim();
    if (!trimmed) return;
    await updateProviderModel(selected.name, oldId, { id: trimmed, display_name: model.display_name?.trim() || undefined });
  };

  const handleRemoveModel = async (modelId: string) => {
    if (!selected) return;
    await removeProviderModel(selected.name, modelId);
  };

  // --- Claude tier mapping ---
  const handleTierChange = async (tier: 'opus' | 'sonnet' | 'haiku', modelId: string | null) => {
    if (!selected) return;
    const current = selected.claude_model_mapping ?? {};
    const next: ClaudeModelMapping = { ...current, [tier]: modelId };
    await setClaudeModelMapping(selected.name, next);
  };

  // --- Refresh ---
  const handleRefresh = () => {
    void refreshProviders();
  };

  // Per-model probe results, keyed by model id. Cleared when the active
  // provider changes so we never display stale results from a previous
  // selection.
  //
  // The backend Tauri command returns `Result<ProbeModelResult, String>`,
  // so a rejected promise here means the Rust side returned `Err(String)`.
  // The most common cause in production is the per-process rate-limit gate
  // in `apps/remote-code-gui/src-tauri/src/desktop/provider_commands.rs`
  // (1 s minimum interval between probes — see PROBE_MIN_INTERVAL).
  // Other possible errors: HTTP transport error, model-not-found, key
  // rejected.
  //
  // Error UX policy:
  //   1. Surface the message in an inline `probeError` banner so the
  //      user sees what went wrong without leaving the dialog.
  //   2. Coalesce identical rate-limit messages within 2 s, so 5
  //      spam-clicks produce 1 banner instead of 5.
  //   3. Mirror the error to the backend tracing layer via
  //      `recordFrontendLog`, so the Rust-side `daily` log has a
  //      matching record for user-reported issues.
  //   4. Do NOT re-throw. The error is captured into local state
  //      and the banner renders it; re-throwing would let React's
  //      error boundary take over (a much worse experience).
  //   5. The Rust error message is English-only; i18n translation
  //      happens at the i18n-key layer (see docs/dev/probe-error-ux.md
  //      for the design note).
  const handleProbeModel = async (name: string, modelId: string) => {
    try {
      const result = await probeProviderModel(name, modelId);
      setProbeError(null);
      return result;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      const now = Date.now();
      const last = lastProbeErrorRef.current;
      if (!(last && last.message === message && now - last.at < 2000)) {
        lastProbeErrorRef.current = { message, at: now };
        setProbeError(message);
      }
      // Best-effort backend trace; never let this swallow the original
      // error if it fails.
      void recordFrontendLog({
        level: 'warn',
        source: 'SettingsPanel.handleProbeModel',
        message: `probe failed for ${name}/${modelId}: ${message}`,
      }).catch(() => {
        // Tracing is best-effort; do not surface a second error to the
        // user just because the log call failed.
      });
      // Re-throw so callers (e.g. ProviderDetailPanel) can short-circuit
      // their own state updates.  The banner above already shows the
      // message; we are NOT swallowing the failure.
      throw err;
    }
  };

  const clearProbeError = useCallback(() => {
    setProbeError(null);
    lastProbeErrorRef.current = null;
  }, []);

  return (
    <div className="space-y-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="text-sm font-semibold text-rc-text-primary">{t('settings.modelProvider')}</h3>
          <p className="mt-1 text-xs leading-5 text-rc-text-tertiary">{t('settings.modelProviderDesc')}</p>
        </div>
        <button
          onClick={handleRefresh}
          className="inline-flex shrink-0 items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-1.5 text-xs font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
          title={t('settings.refresh')}
        >
          <RefreshCw size={13} />
          {t('settings.refresh')}
        </button>
      </div>

      {probeError && (
        <div
          role="alert"
          aria-live="polite"
          className="flex items-start justify-between gap-3 rounded-md border border-rc-status-danger/40 bg-rc-status-danger/10 px-3 py-2 text-xs text-rc-status-danger"
        >
          <span className="break-words">{probeError}</span>
          <button
            type="button"
            onClick={clearProbeError}
            className="shrink-0 rounded p-0.5 text-rc-status-danger/80 transition-colors hover:bg-rc-status-danger/20 hover:text-rc-status-danger"
            title={t('common.dismiss')}
            aria-label={t('common.dismiss')}
          >
            <X size={14} />
          </button>
        </div>
      )}

      <div className="grid min-h-[460px] grid-cols-[280px_minmax(0,1fr)] overflow-hidden rounded-md border border-rc-border-secondary">
        <ProviderListPanel
          providers={allProviders}
          activeProviderName={activeProviderName}
          selectedName={selectedName}
          onSelect={setSelectedName}
          onAddNew={startAddNew}
        />
        <ProviderDetailPanel
          selected={selected}
          editingNew={editingNew}
          form={form}
          showApiKey={showApiKey}
          onEdit={startEdit}
          onCancel={cancelEdit}
          onChangeForm={setForm}
          onToggleApiKey={() => setShowApiKey((s) => !s)}
          onSave={handleSave}
          onDelete={handleDelete}
          onSetActive={(name) => void setActiveProvider(name)}
          onToggleEnabled={(name, enabled) => void setProviderEnabled(name, enabled)}
          onAddModel={handleAddModel}
          onUpdateModel={handleUpdateModel}
          onRemoveModel={handleRemoveModel}
          onTierChange={handleTierChange}
          onProbeModel={handleProbeModel}
        />
      </div>
    </div>
  );
}

interface ProviderListPanelProps {
  providers: ProviderConfig[];
  activeProviderName: string | null;
  selectedName: string | null;
  onSelect: (name: string) => void;
  onAddNew: () => void;
}

function ProviderListPanel({
  providers,
  activeProviderName,
  selectedName,
  onSelect,
  onAddNew,
}: ProviderListPanelProps) {
  const { t } = useTranslation();
  return (
    <div className="flex min-h-0 flex-col border-r border-rc-border-secondary bg-rc-bg-secondary">
      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        <ProviderListGroup
          providers={providers}
          activeProviderName={activeProviderName}
          selectedName={selectedName}
          onSelect={onSelect}
        />
        {providers.length === 0 && (
          <div className="rounded-md border border-dashed border-rc-border-primary px-3 py-4 text-xs text-rc-text-secondary">
            {t('settings.noProvidersYet')}
          </div>
        )}
      </div>
      <div className="border-t border-rc-border-secondary p-2">
        <button
          onClick={onAddNew}
          className="inline-flex w-full items-center justify-center gap-2 rounded-md border border-dashed border-rc-border-primary px-3 py-2 text-xs font-medium text-rc-text-secondary transition-colors hover:border-rc-border-hover hover:text-rc-text-primary"
        >
          <Plus size={13} />
          {t('settings.addProvider')}
        </button>
      </div>
    </div>
  );
}

function ProviderListGroup({
  providers,
  activeProviderName,
  selectedName,
  onSelect,
}: {
  providers: ProviderConfig[];
  activeProviderName: string | null;
  selectedName: string | null;
  onSelect: (name: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="space-y-1">
        {providers.map((provider) => {
          const isSelected = provider.name === selectedName;
          const isActive = provider.name === activeProviderName;
          const enabled = provider.enabled !== false;
          return (
            <button
              key={provider.name}
              data-testid={`provider-row-${provider.name}`}
              onClick={() => onSelect(provider.name)}
              className={`flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm transition-colors ${
                isSelected
                  ? 'bg-rc-bg-active text-rc-text-primary'
                  : 'text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary'
              }`}
            >
              <span
                aria-hidden
                className={`inline-block h-2 w-2 shrink-0 rounded-full ${
                  enabled ? 'bg-rc-accent-success' : 'bg-rc-text-tertiary'
                }`}
              />
              <span className="truncate font-medium">{provider.name}</span>
              {isActive && (
                <span className="ml-auto inline-flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-rc-accent-primary text-white">
                  <Check size={10} />
                </span>
              )}
              {provider.api_key_stored && !isActive && (
                <span
                  className="ml-auto inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-rc-accent-primary"
                  title={t('settings.keychainStored')}
                />
              )}
            </button>
          );
        })}
    </div>
  );
}

interface ProviderDetailPanelProps {
  selected: ProviderConfig | null;
  editingNew: boolean;
  form: ProviderConfig;
  showApiKey: boolean;
  onEdit: (provider: ProviderConfig) => void;
  onCancel: () => void;
  onChangeForm: (form: ProviderConfig) => void;
  onToggleApiKey: () => void;
  onSave: () => Promise<void>;
  onDelete: (name: string) => Promise<void>;
  onSetActive: (name: string) => void;
  onToggleEnabled: (name: string, enabled: boolean) => void;
  onAddModel: (model: ProviderModel) => Promise<void>;
  onUpdateModel: (oldId: string, model: ProviderModel) => Promise<void>;
  onRemoveModel: (modelId: string) => Promise<void>;
  onTierChange: (tier: 'opus' | 'sonnet' | 'haiku', modelId: string | null) => Promise<void>;
  onProbeModel: (name: string, modelId: string) => Promise<ProbeModelResult>;
}

function ProviderDetailPanel({
  selected,
  editingNew,
  form,
  showApiKey,
  onEdit,
  onCancel,
  onChangeForm,
  onToggleApiKey,
  onSave,
  onDelete,
  onSetActive,
  onToggleEnabled,
  onAddModel,
  onUpdateModel,
  onRemoveModel,
  onTierChange,
  onProbeModel,
}: ProviderDetailPanelProps) {
  const { t } = useTranslation();
  const isEditing = editingNew || (selected != null && !selected.api_key_stored && false);
  // Edit form applies when the user clicked edit (the `editingNew` flag is
  // only true for newly-added providers; existing providers are edited in place
  // by mutating `form`).
  const inEditMode = editingNew || (selected != null && form.name === selected.name && form !== selected);

  if (selected == null) {
    return (
      <div className="flex min-h-0 items-center justify-center bg-rc-bg-chat p-8 text-sm text-rc-text-tertiary">
        {editingNew ? t('settings.newProvider') : t('settings.noProvidersYet')}
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-col bg-rc-bg-chat" data-testid="provider-detail-panel">
      <ProviderDetailHeader
        selected={selected}
        form={form}
        inEditMode={inEditMode}
        onEdit={() => onEdit(selected)}
        onCancel={onCancel}
        onChangeForm={onChangeForm}
        onSetActive={() => onSetActive(selected.name)}
        onToggleEnabled={(enabled) => onToggleEnabled(selected.name, enabled)}
        onDelete={() => void onDelete(selected.name)}
      />

      <div className="min-h-0 flex-1 overflow-y-auto p-5">
        <div className="space-y-4">
          {inEditMode ? (
            <ProviderEditForm
              form={form}
              showApiKey={showApiKey}
              onChangeForm={onChangeForm}
              onToggleApiKey={onToggleApiKey}
              onSave={() => void onSave()}
              onCancel={onCancel}
            />
          ) : (
            <ProviderReadOnlyView
              selected={selected}
              onAddModel={onAddModel}
              onUpdateModel={onUpdateModel}
              onRemoveModel={onRemoveModel}
              onTierChange={onTierChange}
              onProbeModel={onProbeModel}
            />
          )}
        </div>
      </div>
    </div>
  );
}

function ProviderDetailHeader({
  selected,
  form,
  inEditMode,
  onEdit,
  onCancel,
  onChangeForm,
  onSetActive,
  onToggleEnabled,
  onDelete,
}: {
  selected: ProviderConfig;
  form: ProviderConfig;
  inEditMode: boolean;
  onEdit: () => void;
  onCancel: () => void;
  onChangeForm: (form: ProviderConfig) => void;
  onSetActive: () => void;
  onToggleEnabled: (enabled: boolean) => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const isBuiltin = selected.group === 'builtin';
  const enabled = selected.enabled !== false;
  return (
    <div className="flex items-center justify-between gap-3 border-b border-rc-border-secondary px-5 py-3">
      <div className="flex min-w-0 items-center gap-2.5">
        <span
          aria-hidden
          className={`inline-block h-2.5 w-2.5 shrink-0 rounded-full ${
            enabled ? 'bg-rc-accent-success' : 'bg-rc-text-tertiary'
          }`}
        />
        <span className="truncate text-base font-semibold text-rc-text-primary" data-testid="provider-detail-name">
          {selected.name}
        </span>
        {enabled && (
          <span className="inline-flex items-center gap-1 rounded-full bg-rc-accent-success-bg px-2 py-0.5 text-[11px] font-medium text-rc-accent-success">
            {t('settings.enabled')}
          </span>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        {!inEditMode && (
          <button
            onClick={onEdit}
            className="rounded-md p-1.5 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
            title={t('common.edit')}
            data-testid="provider-edit-btn"
          >
            <Pencil size={15} />
          </button>
        )}
        <button
          onClick={() => onToggleEnabled(!enabled)}
          className={`rounded-md border px-2.5 py-1 text-xs font-medium transition-colors ${
            enabled
              ? 'border-rc-border-primary text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary'
              : 'border-rc-accent-primary bg-rc-accent-primary text-white hover:bg-rc-accent-primary-hover'
          }`}
          data-testid="provider-enable-btn"
        >
          {enabled ? t('settings.disable') : t('settings.enable')}
        </button>
        {!isBuiltin && (
          <button
            onClick={onDelete}
            className="rounded-md p-1.5 text-rc-text-tertiary transition-colors hover:bg-rc-accent-error-bg hover:text-rc-accent-error"
            title={t('common.delete')}
            data-testid="provider-delete-btn"
          >
            <Trash2 size={15} />
          </button>
        )}
      </div>
    </div>
  );
}

function ProviderReadOnlyView({
  selected,
  onAddModel,
  onUpdateModel,
  onRemoveModel,
  onTierChange,
  onProbeModel,
}: {
  selected: ProviderConfig;
  onAddModel: (model: ProviderModel) => Promise<void>;
  onUpdateModel: (oldId: string, model: ProviderModel) => Promise<void>;
  onRemoveModel: (modelId: string) => Promise<void>;
  onTierChange: (tier: 'opus' | 'sonnet' | 'haiku', modelId: string | null) => Promise<void>;
  onProbeModel: (name: string, modelId: string) => Promise<ProbeModelResult>;
}) {
  const { t } = useTranslation();
  const [editingModel, setEditingModel] = useState<{ oldId: string; id: string; display_name: string } | null>(null);
  const [newModelId, setNewModelId] = useState('');
  // Map of model_id -> latest probe result, plus a `probing` set to disable
  // the button while the request is in flight.
  const [probeResults, setProbeResults] = useState<Record<string, ProbeModelResult | null>>({});
  const [probingIds, setProbingIds] = useState<Set<string>>(new Set());

  // Reset probe state when the active provider changes — probe results
  // belong to a specific (provider, model) pair and would mislead the user
  // if shown against a different provider.
  useEffect(() => {
    setProbeResults({});
    setProbingIds(new Set());
  }, [selected.name]);

  const handleProbe = async (modelId: string) => {
    recordRecent(modelId);
    setProbingIds((prev) => new Set(prev).add(modelId));
    try {
      const result = await onProbeModel(selected.name, modelId);
      setProbeResults((prev) => ({ ...prev, [modelId]: result }));
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setProbeResults((prev) => ({
        ...prev,
        [modelId]: {
          model_id: modelId,
          url: '',
          outcome: 'transport_error',
          detail,
          status_code: null,
          latency_ms: 0,
          agents: [],
        },
      }));
    } finally {
      setProbingIds((prev) => {
        const next = new Set(prev);
        next.delete(modelId);
        return next;
      });
    }
  };

  const models = selected.models ?? [];
  const mapping = selected.claude_model_mapping ?? {};
  const modelOptions = models.length > 0 ? models : (selected.model ? [{ id: selected.model }] : []);

  // --- P3 #22: recent models, persisted in localStorage per provider ---
  const RECENT_KEY = `rc-provider-recent-${selected.name}`;
  const [recent, setRecent] = useState<string[]>(() => {
    try {
      const raw = localStorage.getItem(RECENT_KEY);
      return raw ? (JSON.parse(raw) as string[]) : [];
    } catch { return []; }
  });
  const recordRecent = (modelId: string) => {
    setRecent((prev) => {
      const next = [modelId, ...prev.filter((m) => m !== modelId)].slice(0, 5);
      try { localStorage.setItem(RECENT_KEY, JSON.stringify(next)); } catch { /* ignore */ }
      return next;
    });
  };
  // Reset recent when the active provider changes.
  useEffect(() => {
    try {
      const raw = localStorage.getItem(RECENT_KEY);
      setRecent(raw ? (JSON.parse(raw) as string[]) : []);
    } catch { setRecent([]); }
  }, [selected.name]); // eslint-disable-line react-hooks/exhaustive-deps
  const recentModels = recent
    .map((id) => models.find((m) => m.id === id))
    .filter((m): m is NonNullable<typeof m> => Boolean(m));

  return (
    <>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label={t('settings.anthropicBaseUrl')}>
          <ReadonlyValue value={selected.anthropic_base_url ?? selected.base_url ?? null} placeholder="—" />
        </Field>
        <Field label={t('settings.openaiBaseUrl')}>
          <ReadonlyValue value={selected.openai_base_url ?? null} placeholder="—" />
        </Field>
        <Field label="API Key">
          <span className="text-sm text-rc-text-secondary">
            {selected.api_key_stored
              ? `••••••••  ${t('settings.keychainStored')}`
              : t('settings.unset')}
          </span>
        </Field>
        <Field label={t('settings.fieldDefaultModel')}>
          <ReadonlyValue value={selected.model ?? null} placeholder={t('settings.unset')} />
        </Field>
      </div>

      <div className="rounded-md border border-rc-border-secondary bg-rc-bg-surface p-3">
        <div className="mb-2 flex items-center justify-between">
          <span className="text-sm font-medium text-rc-text-primary">{t('settings.modelList')}</span>
          <span className="text-xs text-rc-text-tertiary">{models.length}</span>
        </div>
        {recentModels.length > 0 && (
          <div className="mb-2" data-testid="recent-models">
            <div className="mb-1 text-[11px] font-medium uppercase tracking-wide text-rc-text-tertiary">
              {t('settings.recentModels')}
            </div>
            <div className="flex flex-wrap gap-1.5">
              {recentModels.map((m) => (
                <button
                  key={`recent-${m.id}`}
                  type="button"
                  onClick={() => recordRecent(m.id)}
                  data-testid={`recent-model-${m.id}`}
                  className="inline-flex items-center gap-1 rounded-full border border-rc-accent-primary/30 bg-rc-bg-active px-2.5 py-1 text-[11px] font-medium text-rc-text-primary hover:border-rc-accent-primary"
                >
                  <span>{m.display_name ?? m.id}</span>
                </button>
              ))}
            </div>
          </div>
        )}
        <div className="space-y-1.5">
          {models.map((model) =>
            editingModel && editingModel.oldId === model.id ? (
              <ModelRowEditor
                key={model.id}
                initialId={model.id}
                initialDisplayName={model.display_name ?? ''}
                onCancel={() => setEditingModel(null)}
                onSave={(next) => {
                  void onUpdateModel(model.id, { id: next.id, display_name: next.display_name });
                  setEditingModel(null);
                }}
              />
            ) : (
              <div
                key={model.id}
                className="rounded-md border border-rc-border-secondary bg-rc-bg-secondary px-3 py-1.5"
              >
                <div className="flex items-center gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm text-rc-text-primary">{model.id}</div>
                    {model.display_name && (
                      <div className="truncate text-xs text-rc-text-tertiary">{model.display_name}</div>
                    )}
                  </div>
                  <button
                    className="rounded-md p-1 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary disabled:cursor-not-allowed disabled:opacity-50"
                    onClick={() => void handleProbe(model.id)}
                    title={t('settings.probeModel')}
                    disabled={probingIds.has(model.id)}
                    data-testid={`probe-model-${model.id}`}
                  >
                    <Plug size={13} className={probingIds.has(model.id) ? 'animate-pulse' : ''} />
                  </button>
                  <button
                    className="rounded-md p-1 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
                    onClick={() =>
                      setEditingModel({ oldId: model.id, id: model.id, display_name: model.display_name ?? '' })
                    }
                    title={t('common.edit')}
                  >
                    <Pencil size={13} />
                  </button>
                  <button
                    className="rounded-md p-1 text-rc-text-tertiary transition-colors hover:bg-rc-accent-error-bg hover:text-rc-accent-error"
                    onClick={() => void onRemoveModel(model.id)}
                    title={t('common.delete')}
                    data-testid={`remove-model-${model.id}`}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
                <ModelProbeChips
                  result={probeResults[model.id] ?? null}
                  probing={probingIds.has(model.id)}
                  t={t}
                />
              </div>
            ),
          )}

          <div className="flex items-center gap-2 pt-1">
            <input
              value={newModelId}
              onChange={(e) => setNewModelId(e.target.value)}
              placeholder={t('settings.modelIdPlaceholder')}
              className="min-w-0 flex-1 rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-1.5 text-sm text-rc-text-primary outline-none focus:border-rc-border-focus"
              data-testid="add-model-input"
            />
            <button
              onClick={() => {
                const id = newModelId.trim();
                if (!id) return;
                void onAddModel({ id });
                setNewModelId('');
              }}
              className="inline-flex items-center gap-1.5 rounded-md border border-dashed border-rc-border-primary px-3 py-1.5 text-xs font-medium text-rc-text-secondary transition-colors hover:border-rc-border-hover hover:text-rc-text-primary"
              data-testid="add-model-btn"
            >
              <Plus size={13} />
              {t('settings.addModel')}
            </button>
          </div>
        </div>
      </div>

      <div className="rounded-md border border-rc-border-secondary bg-rc-bg-surface p-3">
        <div className="mb-2 flex items-center gap-1.5 text-sm font-medium text-rc-text-primary">
          <ChevronDown size={14} />
          {t('settings.claudeModelMapping')}
        </div>
        <p className="mb-3 text-xs leading-5 text-rc-text-tertiary">
          {t('settings.claudeModelMappingHint')}
        </p>
        <div className="grid gap-2 sm:grid-cols-3">
          <TierSelect
            label={t('settings.opusTask')}
            value={mapping.opus ?? ''}
            options={modelOptions}
            onChange={(id) => void onTierChange('opus', id || null)}
            data-testid="tier-opus"
          />
          <TierSelect
            label={t('settings.sonnetTask')}
            value={mapping.sonnet ?? ''}
            options={modelOptions}
            onChange={(id) => void onTierChange('sonnet', id || null)}
            data-testid="tier-sonnet"
          />
          <TierSelect
            label={t('settings.haikuTask')}
            value={mapping.haiku ?? ''}
            options={modelOptions}
            onChange={(id) => void onTierChange('haiku', id || null)}
            data-testid="tier-haiku"
          />
        </div>
      </div>
    </>
  );
}

type ProbeTfn = (key: string, options?: Record<string, unknown>) => string;

function ModelProbeChips({
  result,
  probing,
  t,
}: {
  result: ProbeModelResult | null;
  probing: boolean;
  t: ProbeTfn;
}) {
  // No probe yet — render an "Unknown" stub row so the layout doesn't jump
  // when the user clicks the plug icon.
  if (!result) {
    return (
      <div className="mt-1.5 space-y-1 text-[11px] leading-4">
        <div className="flex flex-wrap items-center gap-1.5 text-rc-text-tertiary">
          <span className="font-medium">{t('settings.availableAgents')}:</span>
          <span className="italic">{probing ? t('settings.probing') : t('settings.unprobed')}</span>
        </div>
      </div>
    );
  }
  const available = result.agents.filter((a) => a.available);
  const unavailable = result.agents.filter((a) => !a.available);
  return (
    <div className="mt-1.5 space-y-1 text-[11px] leading-4">
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="font-medium text-rc-text-tertiary">{t('settings.availableAgents')}:</span>
        {available.length === 0 ? (
          <span className="text-rc-text-tertiary">-</span>
        ) : (
          available.map((a) => (
            <span
              key={a.agent_type}
              data-testid={`probe-agent-${a.agent_type}-ok`}
              className="inline-flex items-center rounded border border-rc-accent-success/40 bg-rc-accent-success-bg px-1.5 py-0.5 text-rc-accent-success"
              title={a.detail}
            >
              {a.agent_name}
            </span>
          ))
        )}
      </div>
      {unavailable.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="font-medium text-rc-text-tertiary">{t('settings.unavailableAgents')}:</span>
          {unavailable.map((a) => (
            <span
              key={a.agent_type}
              data-testid={`probe-agent-${a.agent_type}-fail`}
              className="inline-flex items-center rounded border border-rc-accent-error/40 bg-rc-accent-error-bg px-1.5 py-0.5 text-rc-accent-error"
              title={a.detail}
            >
              {a.agent_name}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

function ModelRowEditor({
  initialId,
  initialDisplayName,
  onCancel,
  onSave,
}: {
  initialId: string;
  initialDisplayName: string;
  onCancel: () => void;
  onSave: (next: { id: string; display_name?: string }) => void;
}) {
  const [id, setId] = useState(initialId);
  const [displayName, setDisplayName] = useState(initialDisplayName);
  const { t } = useTranslation();
  return (
    <div className="flex items-center gap-2 rounded-md border border-rc-accent-primary bg-rc-bg-surface px-3 py-1.5">
      <input
        value={id}
        onChange={(e) => setId(e.target.value)}
        placeholder="model id"
        className="min-w-0 flex-1 rounded-md border border-rc-border-primary bg-rc-bg-secondary px-2 py-1 text-sm text-rc-text-primary outline-none focus:border-rc-border-focus"
      />
      <input
        value={displayName}
        onChange={(e) => setDisplayName(e.target.value)}
        placeholder={t('settings.modelIdPlaceholder')}
        className="min-w-0 flex-1 rounded-md border border-rc-border-primary bg-rc-bg-secondary px-2 py-1 text-sm text-rc-text-primary outline-none focus:border-rc-border-focus"
      />
      <button
        onClick={onCancel}
        className="rounded-md p-1 text-rc-text-tertiary hover:bg-rc-bg-hover hover:text-rc-text-primary"
        title={t('common.cancel')}
      >
        <X size={13} />
      </button>
      <button
        onClick={() => {
          const trimmedId = id.trim();
          if (!trimmedId) return;
          onSave({ id: trimmedId, display_name: displayName.trim() || undefined });
        }}
        className="rounded-md bg-rc-accent-primary px-2 py-1 text-xs font-medium text-white hover:bg-rc-accent-primary-hover"
      >
        {t('common.save')}
      </button>
    </div>
  );
}

function TierSelect({
  label,
  value,
  options,
  onChange,
  ...rest
}: {
  label: string;
  value: string;
  options: Array<{ id: string; display_name?: string | null }>;
  onChange: (id: string) => void;
  'data-testid'?: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="space-y-1">
      <label className="text-xs font-medium text-rc-text-secondary">{label}</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        data-testid={rest['data-testid']}
        className="w-full appearance-none rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-1.5 text-sm text-rc-text-primary outline-none focus:border-rc-border-focus"
      >
        <option value="">{t('settings.unset')}</option>
        {options.map((m) => (
          <option key={m.id} value={m.id}>
            {m.display_name ?? m.id}
          </option>
        ))}
      </select>
    </div>
  );
}

function ReadonlyValue({ value, placeholder }: { value: string | null; placeholder: string }) {
  if (!value) return <span className="text-sm text-rc-text-tertiary">{placeholder}</span>;
  return <span className="break-all text-sm text-rc-text-primary">{value}</span>;
}

function ProviderEditForm({
  form,
  showApiKey,
  onChangeForm,
  onToggleApiKey,
  onSave,
  onCancel,
}: {
  form: ProviderConfig;
  showApiKey: boolean;
  onChangeForm: (form: ProviderConfig) => void;
  onToggleApiKey: () => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label={t('settings.fieldName')}>
          <input
            value={form.name}
            onChange={(e) => onChangeForm({ ...form, name: e.target.value })}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
            placeholder="my-provider"
          />
        </Field>
        <Field label={t('settings.fieldProtocol')}>
          <select
            value={form.protocol}
            onChange={(e) => onChangeForm({ ...form, protocol: e.target.value })}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          >
            {protocols(t).map((p) => (
              <option key={p.value} value={p.value}>
                {p.label}
              </option>
            ))}
          </select>
        </Field>
        <Field label={t('settings.anthropicBaseUrl')} hint={t('settings.urlNormalizationHint')}>
          <input
            value={form.anthropic_base_url ?? ''}
            onChange={(e) => onChangeForm({ ...form, anthropic_base_url: e.target.value })}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
            placeholder={t('settings.anthropicBaseUrlPlaceholder')}
          />
        </Field>
        <Field label={t('settings.openaiBaseUrl')} hint={t('settings.urlNormalizationHint')}>
          <input
            value={form.openai_base_url ?? ''}
            onChange={(e) => onChangeForm({ ...form, openai_base_url: e.target.value })}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
            placeholder={t('settings.openaiBaseUrlPlaceholder')}
          />
        </Field>
        <Field label={t('settings.fieldDefaultModel')}>
          <input
            value={form.model ?? ''}
            onChange={(e) => onChangeForm({ ...form, model: e.target.value })}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
            placeholder="glm-5.1"
          />
        </Field>
        <Field
          label="API Key"
          hint={
            form.api_key_stored
              ? t('settings.keyStoredHint')
              : t('settings.keyNewHint')
          }
        >
          <div className="relative">
            <input
              type={showApiKey ? 'text' : 'password'}
              value={form.api_key ?? ''}
              onChange={(e) => onChangeForm({ ...form, api_key: e.target.value })}
              className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 pr-11 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
              placeholder={form.api_key_stored ? t('settings.keyPlaceholder') : 'sk-...'}
            />
            <button
              className="absolute right-3 top-1/2 -translate-y-1/2 text-rc-text-tertiary transition-colors hover:text-rc-text-primary"
              onClick={onToggleApiKey}
              type="button"
            >
              {showApiKey ? <EyeOff size={16} /> : <Eye size={16} />}
            </button>
          </div>
        </Field>
      </div>

      <div className="flex justify-end gap-3">
        <button
          onClick={onCancel}
          className="rounded-md px-4 py-2 text-sm font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          {t('settings.cancelBtn')}
        </button>
        <button
          onClick={onSave}
          className="rounded-md bg-rc-accent-primary px-5 py-2 text-sm font-medium text-white transition-colors hover:bg-rc-accent-primary-hover"
        >
          {t('settings.saveProvider')}
        </button>
      </div>
    </div>
  );
}

function RuntimeTab({
  current,
  onChange,
}: {
  current: FullSettings;
  onChange: (key: keyof FullSettings, value: unknown) => void;
}) {
  const { t } = useTranslation();
  const runtimePaths = current.runtime_paths;

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-sm font-semibold text-rc-text-primary">{t('settings.runtimeParams')}</h3>
      </div>

      <section className="space-y-3">
        <div className="flex items-end justify-between gap-3">
          <div>
            <h4 className="text-sm font-semibold text-rc-text-primary">{t('settings.installDataDir')}</h4>
          </div>
        </div>

        <div className="overflow-hidden rounded-md border border-rc-border-secondary bg-rc-bg-secondary">
          <RuntimePathRow t={t} label="Remote Code Home" path={runtimePaths.profile_dir} />
          <RuntimePathRow t={t} label="Sessions" path={runtimePaths.sessions_dir} />
          <RuntimePathRow t={t} label="Artifacts" path={runtimePaths.artifacts_dir} />
          <RuntimePathRow t={t} label="Logs" path={runtimePaths.logs_dir} />
          <RuntimePathRow t={t} label="Cache" path={runtimePaths.cache_dir} />
          <RuntimePathRow t={t} label="Agents" path={runtimePaths.agents_dir} />
          <RuntimePathRow t={t} label="Remote Control" path={runtimePaths.remote_control_file} />
          <RuntimePathRow t={t} label="Projects DB" path={runtimePaths.gui_projects_file} />
          <RuntimePathRow t={t} label="Providers DB" path={runtimePaths.gui_providers_file} />
          <RuntimePathRow t={t} label="Settings DB" path={runtimePaths.gui_settings_file} />
        </div>
      </section>

      <div className="grid gap-4 sm:grid-cols-2">
        <Field label={t('settings.maxOutputTokens')} hint={t('settings.maxOutputTokensHint')}>
          <input
            type="number"
            min={1}
            value={current.max_output_tokens}
            onChange={(event) => onChange('max_output_tokens', Number(event.target.value))}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          />
        </Field>

        <Field label={t('settings.thinkingBudget')} hint={t('settings.thinkingBudgetHint')}>
          <input
            type="number"
            min={0}
            value={current.thinking_budget ?? ''}
            placeholder="null"
            onChange={(event) => {
              const val = event.target.value;
              onChange('thinking_budget', val === '' ? null : Number(val));
            }}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          />
        </Field>

        <Field label={t('settings.maxTurns')} hint={t('settings.maxTurnsHint')}>
          <input
            type="number"
            min={1}
            value={current.max_turns}
            onChange={(event) => onChange('max_turns', Number(event.target.value))}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          />
        </Field>

        <Field label={t('settings.requestTimeoutMs')}>
          <input
            type="number"
            min={1000}
            value={current.timeout_ms}
            onChange={(event) => onChange('timeout_ms', Number(event.target.value))}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          />
        </Field>

        <Field label={t('settings.maxRetries')}>
          <input
            type="number"
            min={0}
            value={current.max_retries}
            onChange={(event) => onChange('max_retries', Number(event.target.value))}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          />
        </Field>

        <Field label={t('settings.initialBackoffMs')}>
          <input
            type="number"
            min={50}
            value={current.retry_initial_backoff_ms}
            onChange={(event) => onChange('retry_initial_backoff_ms', Number(event.target.value))}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          />
        </Field>
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        <Field label={t('settings.maxBackoffMs')}>
          <input
            type="number"
            min={50}
            value={current.retry_max_backoff_ms}
            onChange={(event) => onChange('retry_max_backoff_ms', Number(event.target.value))}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          />
        </Field>
      </div>

      <Field label={t('settings.otherSection')}>
        <div className="space-y-2">
          <label className="flex items-center gap-3 rounded-md border border-rc-border-secondary bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary transition-colors hover:border-rc-border-hover">
            <div className="flex h-4 w-4 shrink-0 items-center justify-center rounded border border-rc-border-primary">
              {current.respect_retry_after && <Check size={12} className="text-rc-accent-primary" />}
            </div>
            <input
              type="checkbox"
              checked={current.respect_retry_after}
              onChange={(event) => onChange('respect_retry_after', event.target.checked)}
              className="sr-only"
            />
            <span>{t('settings.enableRetryAfter')}</span>
          </label>

          <label className="flex items-center gap-3 rounded-md border border-rc-border-secondary bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary transition-colors hover:border-rc-border-hover">
            <div className="flex h-4 w-4 shrink-0 items-center justify-center rounded border border-rc-border-primary">
              {current.verbose && <Check size={12} className="text-rc-accent-primary" />}
            </div>
            <input
              type="checkbox"
              checked={current.verbose}
              onChange={(event) => onChange('verbose', event.target.checked)}
              className="sr-only"
            />
            <span>{t('settings.enableVerboseMode')}</span>
          </label>
        </div>
      </Field>
    </div>
  );
}
