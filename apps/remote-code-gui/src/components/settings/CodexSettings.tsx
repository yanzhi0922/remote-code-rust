import { useMemo, useState } from 'react';
import type { FullSettings } from '../../lib/types';
import { SettingInput } from './SettingInput';
import { ToggleSwitch } from './ToggleSwitch';

export interface CodexSettingsProps {
  settings: FullSettings;
  onUpdate: (updates: Record<string, unknown>) => void;
}

const APPROVAL_POLICIES = [
  { value: '', label: '跟随 GUI 权限模式' },
  { value: 'unless-trusted', label: 'Unless trusted' },
  { value: 'on-request', label: 'On request' },
  { value: 'on-failure', label: 'On failure' },
  { value: 'never', label: 'Never' },
];

const SANDBOX_MODES = [
  { value: '', label: '跟随 GUI 权限模式' },
  { value: 'read-only', label: 'Read only' },
  { value: 'workspace-write', label: 'Workspace write' },
  { value: 'danger-full-access', label: 'Danger full access' },
];

function parseOverrides(text: string): Record<string, string> {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith('#'))
    .reduce<Record<string, string>>((acc, line) => {
      const index = line.indexOf('=');
      if (index <= 0) return acc;
      const key = line.slice(0, index).trim();
      const value = line.slice(index + 1).trim();
      if (key) acc[key] = value;
      return acc;
    }, {});
}

function formatOverrides(overrides: Record<string, string>): string {
  return Object.entries(overrides)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, value]) => `${key} = ${value}`)
    .join('\n');
}

export function CodexSettings({ settings, onUpdate }: CodexSettingsProps) {
  const [overridesText, setOverridesText] = useState(() =>
    formatOverrides(settings.codex_config_overrides ?? {}),
  );

  const overridesCount = useMemo(
    () => Object.keys(parseOverrides(overridesText)).length,
    [overridesText],
  );

  return (
    <div className="space-y-5" data-testid="codex-settings">
      <div>
        <h3 className="text-sm font-semibold text-rc-text-primary">Codex 原生设置</h3>
        <p className="mt-1 text-xs leading-5 text-rc-text-tertiary">
          这些设置会透传到官方 Codex Config/App Server。留空表示使用 GUI 的通用 Provider/权限映射。
        </p>
      </div>

      <SettingInput
        label="Model provider id"
        description="传给 Codex 的 model_provider；留空时根据当前 GUI provider 自动生成。"
        value={settings.codex_model_provider ?? ''}
        onChange={(value) => onUpdate({ codex_model_provider: value.trim() || null })}
        placeholder="remote-code-openai"
      />

      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-1.5">
          <label className="block text-sm font-medium text-rc-text-primary">Approval policy</label>
          <select
            value={settings.codex_approval_policy ?? ''}
            onChange={(event) =>
              onUpdate({ codex_approval_policy: event.target.value || null })
            }
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary focus:border-rc-border-focus focus:outline-none"
            data-testid="codex-approval-policy"
          >
            {APPROVAL_POLICIES.map((policy) => (
              <option key={policy.value} value={policy.value}>
                {policy.label}
              </option>
            ))}
          </select>
        </div>

        <div className="space-y-1.5">
          <label className="block text-sm font-medium text-rc-text-primary">Sandbox mode</label>
          <select
            value={settings.codex_sandbox_mode ?? ''}
            onChange={(event) => onUpdate({ codex_sandbox_mode: event.target.value || null })}
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary focus:border-rc-border-focus focus:outline-none"
            data-testid="codex-sandbox-mode"
          >
            {SANDBOX_MODES.map((mode) => (
              <option key={mode.value} value={mode.value}>
                {mode.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      <ToggleSwitch
        label="保存扩展历史"
        description="开启 Codex persist_extended_history，保留更完整的线程历史。"
        checked={settings.codex_persist_extended_history}
        onChange={(checked) => onUpdate({ codex_persist_extended_history: checked })}
      />

      <ToggleSwitch
        label="启用 Codex memories"
        description="控制 Codex memories.generate_memories 和 memories.use_memories。"
        checked={settings.codex_memories_enabled}
        onChange={(checked) => onUpdate({ codex_memories_enabled: checked })}
      />

      <SettingInput
        label="Thread store endpoint"
        description="留空使用隔离本地 thread store；填写后使用 Codex remote thread store。"
        value={settings.codex_thread_store_endpoint ?? ''}
        onChange={(value) => onUpdate({ codex_thread_store_endpoint: value.trim() || null })}
        placeholder="https://..."
      />

      <div className="space-y-1.5">
        <div className="flex items-center justify-between gap-3">
          <label className="block text-sm font-medium text-rc-text-primary">
            Config overrides
          </label>
          <span className="text-xs text-rc-text-tertiary">{overridesCount} 条</span>
        </div>
        <textarea
          value={overridesText}
          onChange={(event) => {
            const value = event.target.value;
            setOverridesText(value);
            onUpdate({ codex_config_overrides: parseOverrides(value) });
          }}
          placeholder={'model_reasoning_effort = "medium"\nfeatures.experimental_api = true'}
          className="min-h-40 w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2 font-mono text-xs text-rc-text-primary placeholder:text-rc-text-tertiary focus:border-rc-border-focus focus:outline-none"
          data-testid="codex-config-overrides"
        />
        <p className="text-xs leading-5 text-rc-text-tertiary">
          每行一个 `key = value`。value 会按 TOML scalar 解析；无法解析时按字符串传递。
        </p>
      </div>
    </div>
  );
}
