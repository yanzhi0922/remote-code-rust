import {
  Archive,
  ArchiveRestore,
  Blocks,
  Bot,
  Check,
  Copy,
  Eye,
  EyeOff,
  Gauge,
  Pencil,
  Plus,
  Power,
  SlidersHorizontal,
  TerminalSquare,
  Trash2,
  Wifi,
  X,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { FullSettings, ModelProfile, ProviderConfig, SessionSummary } from '../../lib/types';
import { useAppStore } from '../../stores/useAppStore';
import { CodexSettings } from '../settings/CodexSettings';
import { RemoteTab } from '../settings/RemoteTab';
import { McpTab } from './McpTab';
import { OperationsTab } from './OperationsTab';

interface SettingsPanelProps {
  open: boolean;
  onClose: () => void;
  initialTab?: SettingsTab;
}

export type SettingsTab = 'provider' | 'runtime' | 'codex' | 'mcp' | 'remote' | 'operations' | 'archive';

type TFn = (key: string, options?: Record<string, unknown>) => string;

function settingsTabs(t: TFn): Array<{ key: SettingsTab; label: string; icon: React.ElementType }> {
  return [
    { key: 'provider', label: 'Provider', icon: SlidersHorizontal },
    { key: 'runtime', label: t('settings.runtimeParams'), icon: Gauge },
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

function permissionModes(t: TFn) {
  return [
    { value: 'default', label: t('chatInput.permission.claude.default'), desc: t('chatInput.permission.claude.defaultDesc') },
    { value: 'acceptEdits', label: t('chatInput.permission.claude.acceptEdits'), desc: t('chatInput.permission.claude.acceptEditsDesc') },
    { value: 'dontAsk', label: t('chatInput.permission.claude.dontAsk'), desc: t('chatInput.permission.claude.dontAskDesc') },
    { value: 'bypassPermissions', label: t('chatInput.permission.claude.bypassPermissions'), desc: t('chatInput.permission.claude.bypassPermissionsDesc') },
    { value: 'plan', label: t('chatInput.permission.claude.plan'), desc: t('chatInput.permission.claude.planDesc') },
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
  const switchProfileAction = useAppStore((state) => state.switchProfile);

  const [editingName, setEditingName] = useState<string | null>(null);
  const [form, setForm] = useState<ProviderConfig>(emptyProviderConfig());
  const [showApiKey, setShowApiKey] = useState(false);

  const activeProviderName = providerConfigs?.active_provider;
  const providers = providerConfigs?.providers ?? [];

  const title = useMemo(() => {
    if (editingName === null) return null;
    return editingName === 'new' ? t('settings.newProvider') : t('settings.editProvider', { name: editingName });
  }, [editingName]);

  const startAdd = () => {
    setEditingName('new');
    setForm(emptyProviderConfig());
    setShowApiKey(false);
  };

  const startEdit = (config: ProviderConfig) => {
    setEditingName(config.name);
    setForm({
      ...config,
      base_url: config.base_url ?? '',
      // Never pre-fill API key — it's masked by backend. Track stored status separately.
      api_key: '',
      model: config.model ?? '',
      profiles: config.profiles ? [...config.profiles] : [],
      api_key_stored: config.api_key_stored ?? false,
    });
    setShowApiKey(false);
  };

  const handleSave = async () => {
    if (!form.name.trim()) return;
    // Check for duplicate provider name (excluding the provider being edited)
    const trimmedName = form.name.trim();
    const existingNames = new Set(
      providers.filter((p) => p.name !== editingName).map((p) => p.name),
    );
    const isDuplicate = existingNames.has(trimmedName);
    if (isDuplicate) {
      // eslint-disable-next-line no-alert
      const proceed = window.confirm(
        t('settings.providerExists', { name: trimmedName }),
      );
      if (!proceed) return;
    }
    await saveProviderConfig(
      {
        name: trimmedName,
        protocol: form.protocol,
        base_url: form.base_url?.trim() || undefined,
        api_key: form.api_key?.trim() || undefined,
        model: form.model?.trim() || undefined,
        profiles: form.profiles?.filter((p) => p.name.trim()),
        active_profile: form.active_profile,
      },
      editingName === 'new' || activeProviderName === trimmedName,
    );
    setEditingName(null);
    setForm(emptyProviderConfig());
  };

  // --- Profile editing helpers ---
  const addProfile = () => {
    setForm((state) => ({
      ...state,
      profiles: [...(state.profiles ?? []), { name: '', model: '' }],
    }));
  };

  const removeProfile = (index: number) => {
    setForm((state) => {
      const profiles = [...(state.profiles ?? [])];
      profiles.splice(index, 1);
      return { ...state, profiles };
    });
  };

  const updateProfile = (index: number, field: keyof ModelProfile, value: string) => {
    setForm((state) => {
      const profiles = [...(state.profiles ?? [])];
      profiles[index] = { ...profiles[index], [field]: value || undefined };
      return { ...state, profiles };
    });
  };

  const handleSwitchProfile = useCallback(
    (providerName: string, profileName: string | null) => {
      void switchProfileAction(providerName, profileName);
    },
    [switchProfileAction],
  );

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold text-rc-text-primary">{t('settings.savedProviders')}</h3>
        </div>
        <button
          onClick={startAdd}
          className="inline-flex items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-4 py-2 text-sm font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          <Plus size={15} />
          {t('settings.addProviderBtn')}
        </button>
      </div>

      <div className="space-y-2">
        {providers.length > 0 ? (
          providers.map((provider) => {
            const active = provider.name === activeProviderName;
            const profiles = provider.profiles ?? [];
            const activeProfile = provider.active_profile ?? null;
            // Determine effective model display
            const effectiveModel =
              activeProfile
                ? profiles.find((p) => p.name === activeProfile)?.model ?? provider.model ?? t('settings.noModelSet')
                : provider.model ?? t('settings.noModelSet');

            return (
              <div
                key={provider.name}
                className={`rounded-md border px-3 py-2.5 ${
                  active ? 'border-rc-border-focus bg-rc-bg-surface' : 'border-rc-border-secondary bg-rc-bg-secondary'
                }`}
              >
                <div className="flex items-center gap-3">
                  <button
                    title={active ? t('settings.currentlyActive') : t('settings.setAsCurrent')}
                    className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-md border ${
                      active ? 'border-rc-accent-primary bg-rc-accent-primary text-white' : 'border-rc-border-primary text-rc-text-secondary'
                    }`}
                    onClick={() => {
                      if (!active) {
                        void setActiveProvider(provider.name);
                      }
                    }}
                  >
                    {active ? <Check size={14} /> : <Power size={14} />}
                  </button>

                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-semibold text-rc-text-primary">{provider.name}</div>
                    <div className="mt-1 truncate text-xs text-rc-text-secondary">
                      {[effectiveModel, provider.protocol, provider.base_url]
                        .filter(Boolean)
                        .join(' · ')}
                      {provider.api_key_stored && (
                        <span className="ml-1.5 inline-flex items-center gap-0.5 rounded bg-rc-accent-success-bg px-1.5 py-0.5 text-[10px] font-medium text-rc-accent-success">
                          {t('settings.keychainStored')}
                        </span>
                      )}
                    </div>
                  </div>

                  <button
                    title={t('common.edit')}
                    className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
                    onClick={() => startEdit(provider)}
                  >
                    <Pencil size={15} />
                  </button>
                  <button
                    title={t('common.delete')}
                    className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-accent-error-bg hover:text-rc-accent-error"
                    onClick={() => {
                      void deleteProviderConfig(provider.name);
                    }}
                  >
                    <Trash2 size={15} />
                  </button>
                </div>

                {/* Profile pills */}
                {profiles.length > 0 && (
                  <div className="mt-2.5 flex flex-wrap items-center gap-1.5 pl-11">
                    {/* Default (no profile) pill */}
                    <button
                      onClick={() => handleSwitchProfile(provider.name, null)}
                      className={`inline-flex items-center gap-1 rounded border px-2.5 py-1 text-[11px] font-medium transition-colors ${
                        activeProfile === null
                          ? 'border-rc-accent-primary bg-rc-accent-primary text-white'
                          : 'border-rc-border-primary bg-rc-bg-surface text-rc-text-secondary hover:border-rc-border-hover'
                      }`}
                    >
                      {t('settings.defaultProfile')}
                    </button>
                    {profiles.map((profile) => (
                      <button
                        key={profile.name}
                        onClick={() => handleSwitchProfile(provider.name, profile.name)}
                        className={`inline-flex items-center gap-1 rounded border px-2.5 py-1 text-[11px] font-medium transition-colors ${
                          activeProfile === profile.name
                            ? 'border-rc-accent-primary bg-rc-accent-primary text-white'
                            : 'border-rc-border-primary bg-rc-bg-surface text-rc-text-secondary hover:border-rc-border-hover'
                        }`}
                        title={profile.model ? t('settings.profileTooltip', { model: profile.model }) : profile.name}
                      >
                        {profile.name}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            );
          })
        ) : (
          <div className="rounded-md border border-dashed border-rc-border-primary px-3 py-5 text-sm text-rc-text-secondary">
            {t('settings.noProvidersYet')}
          </div>
        )}
      </div>

      {editingName && (
        <div className="space-y-4 rounded-md border border-rc-border-primary bg-rc-bg-surface p-4">
          <div className="text-sm font-semibold text-rc-text-primary">{title}</div>

          <div className="grid gap-4 sm:grid-cols-2">
            <Field label={t('settings.fieldName')} hint="">
              <input
                value={form.name}
                onChange={(event) => setForm((state) => ({ ...state, name: event.target.value }))}
                disabled={editingName !== 'new'}
                className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
                placeholder="GLM CODING PLAN"
              />
            </Field>

            <Field label={t('settings.fieldProtocol')}>
              <select
                title={t('settings.fieldProtocol')}
                value={form.protocol}
                onChange={(event) => setForm((state) => ({ ...state, protocol: event.target.value }))}
                className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
              >
                {protocols(t).map((protocol) => (
                  <option key={protocol.value} value={protocol.value}>
                    {protocol.label}
                  </option>
                ))}
              </select>
            </Field>

            <Field label="Base URL" hint={t('settings.urlNormalizationHint')}>
              <input
                value={form.base_url ?? ''}
                onChange={(event) =>
                  setForm((state) => ({ ...state, base_url: event.target.value }))
                }
                className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
                placeholder="https://open.bigmodel.cn/api/anthropic"
              />
            </Field>

            <Field label={t('settings.fieldDefaultModel')}>
              <input
                value={form.model ?? ''}
                onChange={(event) => setForm((state) => ({ ...state, model: event.target.value }))}
                className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
                placeholder="glm-5.1"
              />
            </Field>
          </div>

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
                onChange={(event) => setForm((state) => ({ ...state, api_key: event.target.value }))}
                className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 pr-11 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
                placeholder={form.api_key_stored ? t('settings.keyPlaceholder') : 'sk-...'}
              />
              <button
                className="absolute right-3 top-1/2 -translate-y-1/2 text-rc-text-tertiary transition-colors hover:text-rc-text-primary"
                onClick={() => setShowApiKey((state) => !state)}
              >
                {showApiKey ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
          </Field>

          {/* Profile management */}
          <Field label={t('settings.modelConfigLabel')} hint="">
            <div className="space-y-2">
              {(form.profiles ?? []).map((profile, index) => (
                <div key={profile.name || `profile-${index}`} className="flex items-center gap-2">
                  <input
                    value={profile.name}
                    onChange={(event) => updateProfile(index, 'name', event.target.value)}
                    className="w-32 rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
                    placeholder={t('settings.configNamePlaceholder')}
                  />
                  <input
                    value={profile.model ?? ''}
                    onChange={(event) => updateProfile(index, 'model', event.target.value)}
                    className="min-w-0 flex-1 rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
                    placeholder={t('settings.modelIdPlaceholder')}
                  />
                  <button
                    title={t('settings.deleteProfile')}
                    className="shrink-0 rounded-md p-1.5 text-rc-text-tertiary transition-colors hover:bg-rc-accent-error-bg hover:text-rc-accent-error"
                    onClick={() => removeProfile(index)}
                  >
                    <X size={14} />
                  </button>
                </div>
              ))}
              <button
                onClick={addProfile}
                className="inline-flex items-center gap-1.5 rounded-md border border-dashed border-rc-border-primary px-3 py-2 text-xs font-medium text-rc-text-secondary transition-colors hover:border-rc-border-hover hover:text-rc-text-primary"
              >
                <Plus size={13} />
                {t('settings.addProfile')}
              </button>
            </div>
          </Field>

          <div className="flex justify-end gap-3">
            <button
              onClick={() => {
                setEditingName(null);
                setForm(emptyProviderConfig());
              }}
              className="rounded-md px-4 py-2 text-sm font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
            >
              {t('settings.cancelBtn')}
            </button>
            <button
              onClick={() => {
                void handleSave();
              }}
              className="rounded-md bg-rc-accent-primary px-5 py-2 text-sm font-medium text-white transition-colors hover:bg-rc-accent-primary-hover"
            >
              {t('settings.saveProvider')}
            </button>
          </div>
        </div>
      )}
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

      <Field label={t('settings.permissionModeField')}>
        <div className="space-y-2">
          {permissionModes(t).map((mode) => (
            <label
              key={mode.value}
              className={`flex cursor-pointer items-start gap-3 rounded-md border px-3 py-2.5 text-sm transition-colors ${
                current.permission_mode === mode.value
                  ? 'border-rc-border-focus bg-rc-bg-surface'
                  : 'border-rc-border-secondary bg-rc-bg-secondary hover:border-rc-border-hover'
              }`}
            >
              <div className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border border-rc-border-primary">
                {current.permission_mode === mode.value && (
                  <div className="h-2 w-2 rounded-full bg-rc-accent-primary" />
                )}
              </div>
              <input
                type="radio"
                name="permission_mode"
                value={mode.value}
                checked={current.permission_mode === mode.value}
                onChange={(event) => onChange('permission_mode', event.target.value)}
                className="sr-only"
              />
              <div>
                <div className="font-medium text-rc-text-primary">{mode.label}</div>
                <div className="mt-1 text-xs text-rc-text-tertiary">{mode.desc}</div>
              </div>
            </label>
          ))}
        </div>
      </Field>

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

      <Field label={t('settings.rooModeField')} hint={t('settings.rooModeHint')}>
        <div className="grid gap-2 sm:grid-cols-3">
          {[
            { value: '', label: t('settings.rooModeDefault') },
            { value: 'code', label: 'Code' },
            { value: 'architect', label: 'Architect' },
            { value: 'ask', label: 'Ask' },
            { value: 'debug', label: 'Debug' },
            { value: 'orchestrator', label: 'Orchestrator' },
          ].map((mode) => (
            <button
              key={mode.value}
              type="button"
              onClick={() => onChange('roo_mode', mode.value || null)}
              className={`rounded-md border px-3 py-2 text-xs font-medium transition-colors ${
                (current.roo_mode ?? '') === mode.value
                  ? 'border-rc-accent-primary bg-rc-bg-selected text-rc-accent-primary'
                  : 'border-rc-border-secondary bg-rc-bg-secondary text-rc-text-secondary hover:border-rc-border-hover hover:text-rc-text-primary'
              }`}
            >
              {mode.label}
            </button>
          ))}
        </div>
      </Field>

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
