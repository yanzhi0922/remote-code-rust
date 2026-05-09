import { useCallback, useEffect, useState } from 'react';
import { Bot, Info, Palette, Radio, Settings, Shield, Terminal, X } from 'lucide-react';
import type { FullSettings } from '../../lib/types';
import { useAppStore } from '../../stores/useAppStore';
import { AboutPanel } from './AboutPanel';
import { CodexSettings } from './CodexSettings';
import { GeneralSettings } from './GeneralSettings';
import { HooksSettings, type HookConfig } from './HooksSettings';
import { OutputStylePicker } from './OutputStylePicker';
import { PermissionSettings } from './PermissionSettings';
import { RemoteSettings } from './RemoteSettings';
import { ProviderSettings } from './ProviderSettings';
import { ThemePicker } from './ThemePicker';

export interface SettingsPanelProps {
  onClose: () => void;
}

type SettingsTab = 'general' | 'provider' | 'codex' | 'permissions' | 'remote' | 'appearance' | 'hooks' | 'about';

const TABS: Array<{ key: SettingsTab; label: string; icon: typeof Settings }> = [
  { key: 'general', label: '通用', icon: Settings },
  { key: 'provider', label: '提供商', icon: Terminal },
  { key: 'codex', label: 'Codex', icon: Bot },
  { key: 'permissions', label: '权限', icon: Shield },
  { key: 'remote', label: '远程控制', icon: Radio },
  { key: 'appearance', label: '外观', icon: Palette },
  { key: 'hooks', label: 'Hooks', icon: Terminal },
  { key: 'about', label: '关于', icon: Info },
];

export function SettingsPanel({ onClose }: SettingsPanelProps) {
  const settings = useAppStore((s) => s.settings);
  const loadSettings = useAppStore((s) => s.loadSettings);
  const updateSettings = useAppStore((s) => s.updateSettings);

  const [activeTab, setActiveTab] = useState<SettingsTab>('general');
  const [draft, setDraft] = useState<Record<string, unknown>>({});
  const [theme, setTheme] = useState('system');
  const [outputStyle, setOutputStyle] = useState('default');
  const [hooks, setHooks] = useState<HookConfig[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

  const handleUpdate = useCallback((updates: Record<string, unknown>) => {
    setDraft((prev) => ({ ...prev, ...updates }));
  }, []);

  const handleSave = useCallback(async () => {
    if (Object.keys(draft).length === 0) return;
    setSaving(true);
    try {
      await updateSettings(draft);
      setDraft({});
    } finally {
      setSaving(false);
    }
  }, [draft, updateSettings]);

  const current = { ...settings, ...draft } as FullSettings;

  return (
    <div className="fixed inset-0 z-50 flex" data-testid="settings-panel">
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/30 backdrop-blur-sm"
        onClick={onClose}
        data-testid="settings-backdrop"
      />

      {/* Panel */}
      <div className="relative ml-auto flex h-full w-full max-w-3xl flex-col bg-white shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-200 px-6 py-4">
          <h2 className="text-lg font-semibold text-slate-800">设置</h2>
          <div className="flex items-center gap-3">
            {Object.keys(draft).length > 0 && (
              <button
                type="button"
                onClick={() => void handleSave()}
                disabled={saving}
                className="rounded-xl bg-blue-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
                data-testid="save-settings-btn"
              >
                {saving ? '保存中...' : '保存更改'}
              </button>
            )}
            <button
              type="button"
              onClick={onClose}
              className="rounded-lg p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
              aria-label="关闭设置"
              data-testid="close-settings-btn"
            >
              <X size={20} />
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="flex flex-1 overflow-hidden">
          {/* Sidebar */}
          <nav className="w-48 shrink-0 border-r border-slate-200 bg-slate-50 p-3">
            <ul className="space-y-1">
              {TABS.map((tab) => {
                const Icon = tab.icon;
                return (
                  <li key={tab.key}>
                    <button
                      type="button"
                      onClick={() => setActiveTab(tab.key)}
                      className={`flex w-full items-center gap-2 rounded-xl px-3 py-2 text-sm font-medium transition-colors ${
                        activeTab === tab.key
                          ? 'bg-blue-100 text-blue-700'
                          : 'text-slate-600 hover:bg-slate-100 hover:text-slate-800'
                      }`}
                      data-testid={`tab-${tab.key}`}
                    >
                      <Icon size={16} />
                      {tab.label}
                    </button>
                  </li>
                );
              })}
            </ul>
          </nav>

          {/* Content */}
          <div className="flex-1 overflow-y-auto p-6">
            {!settings ? (
              <div className="flex items-center justify-center py-12 text-slate-400">
                <p>加载设置中...</p>
              </div>
            ) : (
              <>
                {activeTab === 'general' && (
                  <GeneralSettings settings={current} onUpdate={handleUpdate} />
                )}
                {activeTab === 'provider' && (
                  <ProviderSettings settings={current} onUpdate={handleUpdate} />
                )}
                {activeTab === 'codex' && (
                  <CodexSettings settings={current} onUpdate={handleUpdate} />
                )}
                {activeTab === 'permissions' && (
                  <PermissionSettings settings={current} onUpdate={handleUpdate} />
                )}
                {activeTab === 'remote' && (
                  <RemoteSettings />
                )}
                {activeTab === 'appearance' && (
                  <div className="space-y-8">
                    <ThemePicker value={theme} onChange={setTheme} />
                    <OutputStylePicker value={outputStyle} onChange={setOutputStyle} />
                  </div>
                )}
                {activeTab === 'hooks' && (
                  <HooksSettings hooks={hooks} onUpdate={setHooks} />
                )}
                {activeTab === 'about' && <AboutPanel />}
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
