import { ArchiveRestore, Check, Eye, EyeOff, Pencil, Plus, Power, Trash2, X } from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { FullSettings, ModelProfile, ProviderConfig, SessionSummary } from '../../lib/types';
import { useAppStore } from '../../stores/useAppStore';

interface SettingsPanelProps {
  open: boolean;
  onClose: () => void;
}

type SettingsTab = 'provider' | 'runtime' | 'archive';

const TABS: Array<{ key: SettingsTab; label: string }> = [
  { key: 'provider', label: 'Provider' },
  { key: 'runtime', label: '运行参数' },
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
      <label className="block text-sm font-medium text-slate-700">{label}</label>
      {children}
      {hint && <p className="text-xs leading-5 text-slate-500">{hint}</p>}
    </div>
  );
}

export function SettingsPanel({ open, onClose }: SettingsPanelProps) {
  const settings = useAppStore((state) => state.settings);
  const loadSettings = useAppStore((state) => state.loadSettings);
  const loadProviderConfigs = useAppStore((state) => state.loadProviderConfigs);
  const loadArchivedSessions = useAppStore((state) => state.loadArchivedSessions);
  const updateSettings = useAppStore((state) => state.updateSettings);

  const [activeTab, setActiveTab] = useState<SettingsTab>('provider');
  const [draft, setDraft] = useState<Partial<FullSettings>>({});
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    void Promise.all([loadSettings(), loadProviderConfigs(), loadArchivedSessions()]);
    setDraft({});
    setActiveTab('provider');
  }, [loadArchivedSessions, loadProviderConfigs, loadSettings, open]);

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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/25 p-4 backdrop-blur-[2px]">
      <div className="flex max-h-[90vh] w-full max-w-4xl flex-col overflow-hidden rounded-[32px] border border-[#ddd6c8] bg-[#faf8f3] shadow-[0_28px_80px_rgba(15,23,42,0.24)]">
        <div className="flex items-center justify-between border-b border-[#e8e1d6] px-6 py-5">
          <div>
            <h2 className="text-xl font-semibold text-slate-800">设置</h2>
            <p className="mt-1 text-sm text-slate-500">
              Provider 管理是真实持久化的；会在 GUI 重启后继续保留。
            </p>
          </div>
          <button
            onClick={onClose}
            className="rounded-full p-2 text-slate-400 transition-colors hover:bg-white hover:text-slate-700"
          >
            <X size={18} />
          </button>
        </div>

        <div className="grid min-h-0 flex-1 grid-cols-[180px_1fr]">
          <div className="border-r border-[#e8e1d6] bg-[#f4f0e7] p-4">
            <div className="space-y-1">
              {TABS.map((tab) => (
                <button
                  key={tab.key}
                  onClick={() => setActiveTab(tab.key)}
                  className={`w-full rounded-2xl px-4 py-2.5 text-left text-sm font-medium transition-colors ${
                    activeTab === tab.key
                      ? 'bg-white text-slate-900 shadow-[0_8px_20px_rgba(23,24,26,0.06)]'
                      : 'text-slate-600 hover:bg-white/70 hover:text-slate-900'
                  }`}
                >
                  {tab.label}
                </button>
              ))}
            </div>
          </div>

          <div className="min-h-0 overflow-y-auto px-6 py-6">
            {!settings ? (
              <div className="py-10 text-sm text-slate-500">正在加载设置…</div>
            ) : activeTab === 'provider' ? (
              <ProviderTab />
            ) : activeTab === 'archive' ? (
              <ArchiveTab />
            ) : (
              <RuntimeTab current={current} onChange={applyDraft} />
            )}
          </div>
        </div>

        <div className="flex items-center justify-end gap-3 border-t border-[#e8e1d6] bg-white/60 px-6 py-4">
          <button
            onClick={() => setDraft({})}
            className="rounded-2xl px-4 py-2 text-sm font-medium text-slate-500 transition-colors hover:bg-white hover:text-slate-700"
          >
            重置未保存更改
          </button>
          <button
            onClick={() => {
              void handleSave();
            }}
            disabled={Object.keys(draft).length === 0 || saving}
            className="rounded-2xl bg-[#17181a] px-5 py-2 text-sm font-medium text-white transition-colors hover:bg-[#2b2d31] disabled:cursor-not-allowed disabled:bg-[#c9c2b5]"
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
  onRestore,
}: {
  session: SessionSummary;
  onRestore: () => void;
}) {
  return (
    <div className="flex items-center gap-3 rounded-[24px] border border-transparent bg-[#f5f1e9] px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-semibold text-slate-800">{session.title}</div>
        <div className="mt-1 truncate text-xs text-slate-500">
          {session.cwd}
        </div>
        <div className="mt-1 text-[11px] text-slate-500">
          {session.provider_name}
          {session.model ? ` · ${session.model}` : ''} · {formatRelativeTime(session.updated_at)}
        </div>
      </div>

      <button
        onClick={onRestore}
        className="inline-flex items-center gap-2 rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2 text-sm font-medium text-slate-700 transition-colors hover:bg-[#faf8f3]"
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

  return (
    <div className="space-y-5">
      <div>
        <h3 className="text-base font-semibold text-slate-800">归档会话</h3>
        <p className="mt-1 text-sm text-slate-500">
          归档后的会话不会出现在左侧主树中。恢复时，如果原项目节点已经被移除，会自动补回项目文件夹。
        </p>
      </div>

      {archivedSessions.length > 0 ? (
        <div className="space-y-2">
          {archivedSessions.map((session) => (
            <ArchiveRow
              key={session.id}
              session={session}
              onRestore={() => {
                void restoreSession(session.id);
              }}
            />
          ))}
        </div>
      ) : (
        <div className="rounded-[24px] border border-dashed border-[#ddd6c8] px-4 py-6 text-sm text-slate-500">
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
          <h3 className="text-base font-semibold text-slate-800">已保存的 Provider</h3>
          <p className="mt-1 text-sm text-slate-500">
            这里是会真实持久化的多 Provider 列表。添加后，发送框下方会立刻可以切换。
          </p>
        </div>
        <button
          onClick={startAdd}
          className="inline-flex items-center gap-2 rounded-2xl border border-[#ddd6c8] bg-white px-4 py-2 text-sm font-medium text-slate-700 transition-colors hover:bg-[#faf8f3]"
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
                className={`rounded-[24px] border px-4 py-3 ${
                  active ? 'border-[#d8d1c3] bg-white' : 'border-transparent bg-[#f5f1e9]'
                }`}
              >
                <div className="flex items-center gap-3">
                  <button
                    title={active ? '当前激活' : '设为当前'}
                    className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-full border ${
                      active ? 'border-[#17181a] bg-[#17181a] text-white' : 'border-[#d8d1c3] text-slate-500'
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
                    <div className="truncate text-sm font-semibold text-slate-800">{provider.name}</div>
                    <div className="mt-1 truncate text-xs text-slate-500">
                      {[effectiveModel, provider.protocol, provider.base_url]
                        .filter(Boolean)
                        .join(' · ')}
                      {provider.api_key_stored && (
                        <span className="ml-1.5 inline-flex items-center gap-0.5 rounded-full bg-emerald-50 px-1.5 py-0.5 text-[10px] font-medium text-emerald-700">
                          🔒 钥匙串
                        </span>
                      )}
                    </div>
                  </div>

                  <button
                    title="编辑"
                    className="rounded-full p-2 text-slate-400 transition-colors hover:bg-white hover:text-slate-700"
                    onClick={() => startEdit(provider)}
                  >
                    <Pencil size={15} />
                  </button>
                  <button
                    title="删除"
                    className="rounded-full p-2 text-slate-400 transition-colors hover:bg-[#fff1f0] hover:text-red-500"
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
                      className={`inline-flex items-center gap-1 rounded-full border px-2.5 py-1 text-[11px] font-medium transition-colors ${
                        activeProfile === null
                          ? 'border-[#17181a] bg-[#17181a] text-white'
                          : 'border-[#ddd6c8] bg-white text-slate-600 hover:border-slate-400'
                      }`}
                    >
                      默认
                    </button>
                    {profiles.map((profile) => (
                      <button
                        key={profile.name}
                        onClick={() => handleSwitchProfile(provider.name, profile.name)}
                        className={`inline-flex items-center gap-1 rounded-full border px-2.5 py-1 text-[11px] font-medium transition-colors ${
                          activeProfile === profile.name
                            ? 'border-[#17181a] bg-[#17181a] text-white'
                            : 'border-[#ddd6c8] bg-white text-slate-600 hover:border-slate-400'
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
          <div className="rounded-[24px] border border-dashed border-[#ddd6c8] px-4 py-6 text-sm text-slate-500">
            还没有保存任何 Provider。你可以把 GLM、MiniMax 等端点都保存在这里。
          </div>
        )}
      </div>

      {editingName && (
        <div className="space-y-4 rounded-[28px] border border-[#ddd6c8] bg-white p-5">
          <div className="text-sm font-semibold text-slate-800">{title}</div>

          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="名称" hint="切换时显示的名字，例如 GLM CODING PLAN。">
              <input
                value={form.name}
                onChange={(event) => setForm((state) => ({ ...state, name: event.target.value }))}
                disabled={editingName !== 'new'}
                className="w-full rounded-2xl border border-[#ddd6c8] bg-[#faf8f3] px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
                placeholder="GLM CODING PLAN"
              />
            </Field>

            <Field label="协议">
              <select
                title="协议"
                value={form.protocol}
                onChange={(event) => setForm((state) => ({ ...state, protocol: event.target.value }))}
                className="w-full rounded-2xl border border-[#ddd6c8] bg-[#faf8f3] px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
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
                className="w-full rounded-2xl border border-[#ddd6c8] bg-[#faf8f3] px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
                placeholder="https://open.bigmodel.cn/api/anthropic"
              />
            </Field>

            <Field label="默认模型">
              <input
                value={form.model ?? ''}
                onChange={(event) => setForm((state) => ({ ...state, model: event.target.value }))}
                className="w-full rounded-2xl border border-[#ddd6c8] bg-[#faf8f3] px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
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
                className="w-full rounded-2xl border border-[#ddd6c8] bg-[#faf8f3] px-3 py-2.5 pr-11 text-sm outline-none transition-colors focus:border-slate-500"
                placeholder={form.api_key_stored ? '••••••••（留空保持不变）' : 'sk-...'}
              />
              <button
                className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 transition-colors hover:text-slate-700"
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
                    className="w-32 rounded-xl border border-[#ddd6c8] bg-[#faf8f3] px-3 py-2 text-sm outline-none transition-colors focus:border-slate-500"
                    placeholder="配置名"
                  />
                  <input
                    value={profile.model ?? ''}
                    onChange={(event) => updateProfile(index, 'model', event.target.value)}
                    className="min-w-0 flex-1 rounded-xl border border-[#ddd6c8] bg-[#faf8f3] px-3 py-2 text-sm outline-none transition-colors focus:border-slate-500"
                    placeholder="模型 ID，例如 glm-5.1"
                  />
                  <button
                    title="删除此配置"
                    className="shrink-0 rounded-full p-1.5 text-slate-400 transition-colors hover:bg-[#fff1f0] hover:text-red-500"
                    onClick={() => removeProfile(index)}
                  >
                    <X size={14} />
                  </button>
                </div>
              ))}
              <button
                onClick={addProfile}
                className="inline-flex items-center gap-1.5 rounded-xl border border-dashed border-[#ddd6c8] px-3 py-2 text-xs font-medium text-slate-500 transition-colors hover:border-slate-400 hover:text-slate-700"
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
              className="rounded-2xl px-4 py-2 text-sm font-medium text-slate-500 transition-colors hover:bg-[#f7f4ed] hover:text-slate-700"
            >
              取消
            </button>
            <button
              onClick={() => {
                void handleSave();
              }}
              className="rounded-2xl bg-[#17181a] px-5 py-2 text-sm font-medium text-white transition-colors hover:bg-[#2b2d31]"
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
        <h3 className="text-base font-semibold text-slate-800">运行参数</h3>
        <p className="mt-1 text-sm text-slate-500">
          这里保留真正影响执行体验的参数。模型本身请直接在发送框下方即时切换。
        </p>
      </div>

      <Field label="权限模式">
        <div className="space-y-2">
          {PERMISSION_MODES.map((mode) => (
            <label
              key={mode.value}
              className={`flex cursor-pointer items-start gap-3 rounded-2xl border px-4 py-3 text-sm ${
                current.permission_mode === mode.value
                  ? 'border-[#d8d1c3] bg-white'
                  : 'border-transparent bg-[#f5f1e9]'
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
                <div className="font-medium text-slate-700">{mode.label}</div>
                <div className="mt-1 text-xs text-slate-500">{mode.desc}</div>
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
            className="w-full rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
          />
        </Field>

        <Field label="最大重试次数">
          <input
            type="number"
            min={0}
            value={current.max_retries}
            onChange={(event) => onChange('max_retries', Number(event.target.value))}
            className="w-full rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
          />
        </Field>

        <Field label="初始退避（毫秒）">
          <input
            type="number"
            min={50}
            value={current.retry_initial_backoff_ms}
            onChange={(event) => onChange('retry_initial_backoff_ms', Number(event.target.value))}
            className="w-full rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
          />
        </Field>

        <Field label="最大退避（毫秒）">
          <input
            type="number"
            min={50}
            value={current.retry_max_backoff_ms}
            onChange={(event) => onChange('retry_max_backoff_ms', Number(event.target.value))}
            className="w-full rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
          />
        </Field>
      </div>

      <Field label="其他">
        <div className="space-y-2">
          <label className="flex items-center gap-3 rounded-2xl bg-white px-4 py-3 text-sm text-slate-700">
            <input
              type="checkbox"
              checked={current.respect_retry_after}
              onChange={(event) => onChange('respect_retry_after', event.target.checked)}
            />
            <span>启用服务端 Retry-After 支持</span>
          </label>

          <label className="flex items-center gap-3 rounded-2xl bg-white px-4 py-3 text-sm text-slate-700">
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
