import { ArchiveRestore, Check, Eye, EyeOff, Pencil, Plus, Power, Trash2, X } from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { FullSettings, ModelProfile, ProviderConfig, SessionSummary } from '../../lib/types';
import { useAppStore } from '../../stores/useAppStore';
import { CodexSettings } from '../settings/CodexSettings';
import { McpTab } from './McpTab';
import { OperationsTab } from './OperationsTab';

interface SettingsPanelProps {
  open: boolean;
  onClose: () => void;
  initialTab?: SettingsTab;
}

export type SettingsTab = 'provider' | 'runtime' | 'codex' | 'mcp' | 'operations' | 'archive';

const TABS: Array<{ key: SettingsTab; label: string }> = [
  { key: 'provider', label: 'Provider' },
  { key: 'runtime', label: '运行参数' },
  { key: 'codex', label: 'Codex' },
  { key: 'mcp', label: 'MCP' },
  { key: 'operations', label: '操作面' },
  { key: 'archive', label: '归档' },
];

const PROTOCOLS = [
  { value: 'openai', label: 'OpenAI / 兼容 Chat Completions' },
  { value: 'anthropic', label: 'Anthropic Messages' },
  { value: 'bedrock', label: 'AWS Bedrock' },
  { value: 'vertex', label: 'Google Vertex' },
];

const PERMISSION_MODES = [
  { value: 'default', label: '默认', desc: '读取自动执行，写入和命令需确认' },
  { value: 'acceptEdits', label: '自动编辑', desc: '文件编辑自动执行，命令仍需确认' },
  { value: 'dontAsk', label: '不询问', desc: '仅自动放行低风险读取工具' },
  { value: 'bypassPermissions', label: '全自动', desc: '跳过全部权限确认' },
  { value: 'plan', label: '规划', desc: '只规划，不执行工具' },
];

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

export function SettingsPanel({ open, onClose, initialTab = 'provider' }: SettingsPanelProps) {
  const settings = useAppStore((state) => state.settings);
  const loadSettings = useAppStore((state) => state.loadSettings);
  const loadProviderConfigs = useAppStore((state) => state.loadProviderConfigs);
  const loadArchivedSessions = useAppStore((state) => state.loadArchivedSessions);
  const updateSettings = useAppStore((state) => state.updateSettings);

  const [activeTab, setActiveTab] = useState<SettingsTab>(initialTab);
  const [draft, setDraft] = useState<Partial<FullSettings>>({});
  const [saving, setSaving] = useState(false);

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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-rc-bg-overlay p-4 backdrop-blur-[3px]">
      <div className="flex max-h-[90vh] w-full max-w-6xl flex-col overflow-hidden rounded-md border border-rc-border-primary bg-rc-bg-surface shadow-[0_18px_48px_rgba(0,0,0,0.34)]">
        <div className="flex items-center justify-between border-b border-rc-border-primary px-5 py-3">
          <div>
            <h2 className="text-sm font-semibold uppercase tracking-[0.08em] text-rc-text-secondary">Settings</h2>
          </div>
          <button
            onClick={onClose}
            aria-label="关闭设置"
            className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
          >
            <X size={18} />
          </button>
        </div>

        <div className="grid min-h-0 flex-1 grid-cols-[190px_1fr]">
          <div className="border-r border-rc-border-primary bg-rc-bg-secondary p-3">
            <div className="space-y-1">
              {TABS.map((tab) => (
                <button
                  key={tab.key}
                  onClick={() => setActiveTab(tab.key)}
                  className={`w-full rounded-md px-3 py-2 text-left text-xs font-medium transition-colors ${
                    activeTab === tab.key
                      ? 'bg-rc-bg-active text-rc-text-primary'
                      : 'text-rc-text-secondary hover:bg-rc-bg-hover hover:text-rc-text-primary'
                  }`}
                >
                  {tab.label}
                </button>
              ))}
            </div>
          </div>

          <div className="min-h-0 overflow-y-auto px-5 py-5">
            {!settings ? (
              <div className="py-10 text-sm text-rc-text-secondary">正在加载设置…</div>
            ) : activeTab === 'provider' ? (
              <ProviderTab />
            ) : activeTab === 'mcp' ? (
              <McpTab />
            ) : activeTab === 'codex' ? (
              <CodexSettings
                settings={current}
                onUpdate={(updates) => setDraft((state) => ({ ...state, ...updates }))}
              />
            ) : activeTab === 'operations' ? (
              <OperationsTab />
            ) : activeTab === 'archive' ? (
              <ArchiveTab />
            ) : (
              <RuntimeTab current={current} onChange={applyDraft} />
            )}
          </div>
        </div>

        <div className="flex items-center justify-end gap-3 border-t border-rc-border-primary bg-rc-bg-secondary/80 px-5 py-3">
          <button
            onClick={() => setDraft({})}
            className="rounded-md px-4 py-2 text-sm font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
          >
            重置未保存更改
          </button>
          <button
            onClick={() => {
              void handleSave();
            }}
            disabled={Object.keys(draft).length === 0 || saving}
            className="rounded-md bg-rc-accent-primary px-5 py-2 text-sm font-medium text-white transition-colors hover:bg-rc-accent-primary-hover disabled:cursor-not-allowed disabled:bg-rc-text-tertiary"
          >
            {saving ? '保存中…' : '保存'}
          </button>
        </div>
      </div>
    </div>
  );
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

function ArchiveRow({
  session,
  privacyMode,
  onRestore,
}: {
  session: SessionSummary;
  privacyMode: boolean;
  onRestore: () => void;
}) {
  return (
    <div className="flex items-center gap-3 rounded-md border border-rc-border-secondary bg-rc-bg-secondary px-3 py-2.5">
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-semibold text-rc-text-primary">
          {privacyMode ? '会话已隐藏' : session.title}
        </div>
        <div className="mt-1 truncate text-xs text-rc-text-secondary">
          {privacyMode ? '路径已隐藏' : session.cwd}
        </div>
        <div className="mt-1 text-[11px] text-rc-text-tertiary">
          {session.provider_name}
          {session.model ? ` · ${session.model}` : ''} · {formatRelativeTime(session.updated_at)}
        </div>
      </div>

      <button
        onClick={onRestore}
        className="inline-flex items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2 text-sm font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
      >
        <ArchiveRestore size={14} />
        恢复
      </button>
    </div>
  );
}

function ArchiveTab() {
  const archivedSessions = useAppStore((state) => state.archivedSessions);
  const restoreSession = useAppStore((state) => state.restoreSession);
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);

  return (
    <div className="space-y-5">
      <div>
        <h3 className="text-sm font-semibold text-rc-text-primary">归档会话</h3>
      </div>

      {archivedSessions.length > 0 ? (
        <div className="space-y-2">
          {archivedSessions.map((session) => (
            <ArchiveRow
              key={session.id}
              session={session}
              privacyMode={privacyMode}
              onRestore={() => {
                void restoreSession(session.id);
              }}
            />
          ))}
        </div>
      ) : (
        <div className="rounded-md border border-dashed border-rc-border-primary px-3 py-5 text-sm text-rc-text-secondary">
          当前没有已归档会话。
        </div>
      )}
    </div>
  );
}

function ProviderTab() {
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
    return editingName === 'new' ? '新增 Provider' : `编辑 ${editingName}`;
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
    await saveProviderConfig(
      {
        name: form.name.trim(),
        protocol: form.protocol,
        base_url: form.base_url?.trim() || undefined,
        api_key: form.api_key?.trim() || undefined,
        model: form.model?.trim() || undefined,
        profiles: form.profiles?.filter((p) => p.name.trim()),
        active_profile: form.active_profile,
      },
      editingName === 'new' || activeProviderName === form.name.trim(),
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
          <h3 className="text-sm font-semibold text-rc-text-primary">已保存的 Provider</h3>
        </div>
        <button
          onClick={startAdd}
          className="inline-flex items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-4 py-2 text-sm font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
        >
          <Plus size={15} />
          添加
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
                ? profiles.find((p) => p.name === activeProfile)?.model ?? provider.model ?? '未设模型'
                : provider.model ?? '未设模型';

            return (
              <div
                key={provider.name}
                className={`rounded-md border px-3 py-2.5 ${
                  active ? 'border-rc-border-focus bg-rc-bg-surface' : 'border-rc-border-secondary bg-rc-bg-secondary'
                }`}
              >
                <div className="flex items-center gap-3">
                  <button
                    title={active ? '当前激活' : '设为当前'}
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
                          钥匙串
                        </span>
                      )}
                    </div>
                  </div>

                  <button
                    title="编辑"
                    className="rounded-md p-2 text-rc-text-tertiary transition-colors hover:bg-rc-bg-hover hover:text-rc-text-primary"
                    onClick={() => startEdit(provider)}
                  >
                    <Pencil size={15} />
                  </button>
                  <button
                    title="删除"
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
                      默认
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
                        title={profile.model ? `模型: ${profile.model}` : profile.name}
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
            还没有保存任何 Provider。
          </div>
        )}
      </div>

      {editingName && (
        <div className="space-y-4 rounded-md border border-rc-border-primary bg-rc-bg-surface p-4">
          <div className="text-sm font-semibold text-rc-text-primary">{title}</div>

          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="名称" hint="切换时显示的名字，例如 GLM CODING PLAN。">
              <input
                value={form.name}
                onChange={(event) => setForm((state) => ({ ...state, name: event.target.value }))}
                disabled={editingName !== 'new'}
                className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
                placeholder="GLM CODING PLAN"
              />
            </Field>

            <Field label="协议">
              <select
                title="协议"
                value={form.protocol}
                onChange={(event) => setForm((state) => ({ ...state, protocol: event.target.value }))}
                className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
              >
                {PROTOCOLS.map((protocol) => (
                  <option key={protocol.value} value={protocol.value}>
                    {protocol.label}
                  </option>
                ))}
              </select>
            </Field>

            <Field label="Base URL" hint="保存时后端会按协议自动规范化末尾路径。">
              <input
                value={form.base_url ?? ''}
                onChange={(event) =>
                  setForm((state) => ({ ...state, base_url: event.target.value }))
                }
                className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
                placeholder="https://open.bigmodel.cn/api/anthropic"
              />
            </Field>

            <Field label="默认模型">
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
                ? '密钥已安全存储在系统钥匙串中。留空保持不变，输入新值则覆盖。'
                : '密钥将安全存储在系统钥匙串（Windows Credential Manager / macOS Keychain）中。'
            }
          >
            <div className="relative">
              <input
                type={showApiKey ? 'text' : 'password'}
                value={form.api_key ?? ''}
                onChange={(event) => setForm((state) => ({ ...state, api_key: event.target.value }))}
                className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2.5 pr-11 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
                placeholder={form.api_key_stored ? '••••••••（留空保持不变）' : 'sk-...'}
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
          <Field label="模型配置（Profile）" hint="为同一 Provider 创建多套模型映射，一键切换。">
            <div className="space-y-2">
              {(form.profiles ?? []).map((profile, index) => (
                <div key={index} className="flex items-center gap-2">
                  <input
                    value={profile.name}
                    onChange={(event) => updateProfile(index, 'name', event.target.value)}
                    className="w-32 rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
                    placeholder="配置名"
                  />
                  <input
                    value={profile.model ?? ''}
                    onChange={(event) => updateProfile(index, 'model', event.target.value)}
                    className="min-w-0 flex-1 rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
                    placeholder="模型 ID，例如 glm-5.1"
                  />
                  <button
                    title="删除此配置"
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
                添加配置
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
              取消
            </button>
            <button
              onClick={() => {
                void handleSave();
              }}
              className="rounded-md bg-rc-accent-primary px-5 py-2 text-sm font-medium text-white transition-colors hover:bg-rc-accent-primary-hover"
            >
              保存 Provider
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
  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-sm font-semibold text-rc-text-primary">运行参数</h3>
      </div>

      <Field label="权限模式">
        <div className="space-y-2">
          {PERMISSION_MODES.map((mode) => (
            <label
              key={mode.value}
              className={`flex cursor-pointer items-start gap-3 rounded-md border px-3 py-2.5 text-sm ${
                current.permission_mode === mode.value
                  ? 'border-rc-border-focus bg-rc-bg-surface'
                  : 'border-rc-border-secondary bg-rc-bg-secondary'
              }`}
            >
              <input
                type="radio"
                name="permission_mode"
                value={mode.value}
                checked={current.permission_mode === mode.value}
                onChange={(event) => onChange('permission_mode', event.target.value)}
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
        <Field label="请求超时（毫秒）">
          <input
            type="number"
            min={1000}
            value={current.timeout_ms}
            onChange={(event) => onChange('timeout_ms', Number(event.target.value))}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          />
        </Field>

        <Field label="最大重试次数">
          <input
            type="number"
            min={0}
            value={current.max_retries}
            onChange={(event) => onChange('max_retries', Number(event.target.value))}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          />
        </Field>

        <Field label="初始退避（毫秒）">
          <input
            type="number"
            min={50}
            value={current.retry_initial_backoff_ms}
            onChange={(event) => onChange('retry_initial_backoff_ms', Number(event.target.value))}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          />
        </Field>

        <Field label="最大退避（毫秒）">
          <input
            type="number"
            min={50}
            value={current.retry_max_backoff_ms}
            onChange={(event) => onChange('retry_max_backoff_ms', Number(event.target.value))}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
          />
        </Field>
      </div>

      <Field label="其他">
        <div className="space-y-2">
          <label className="flex items-center gap-3 rounded-md bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary">
            <input
              type="checkbox"
              checked={current.respect_retry_after}
              onChange={(event) => onChange('respect_retry_after', event.target.checked)}
            />
            <span>启用服务端 Retry-After 支持</span>
          </label>

          <label className="flex items-center gap-3 rounded-md bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary">
            <input
              type="checkbox"
              checked={current.verbose}
              onChange={(event) => onChange('verbose', event.target.checked)}
            />
            <span>启用详细模式</span>
          </label>
        </div>
      </Field>
    </div>
  );
}
