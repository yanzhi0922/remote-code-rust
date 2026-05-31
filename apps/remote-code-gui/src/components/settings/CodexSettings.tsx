import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { FullSettings } from '../../lib/types';
import { SettingInput } from './SettingInput';
import { ToggleSwitch } from './ToggleSwitch';

export interface CodexSettingsProps {
  settings: FullSettings;
  onUpdate: (updates: Record<string, unknown>) => void;
}

type TFn = (key: string) => string;

function approvalPolicies(t: TFn) {
  return [
    { value: '', label: t('codexSettings.followGuiPermission') },
    { value: 'unless-trusted', label: 'Unless trusted' },
    { value: 'on-request', label: 'On request' },
    { value: 'on-failure', label: 'On failure' },
    { value: 'never', label: 'Never' },
  ];
}

function sandboxModes(t: TFn) {
  return [
    { value: '', label: t('codexSettings.followGuiSandbox') },
    { value: 'read-only', label: 'Read only' },
    { value: 'workspace-write', label: 'Workspace write' },
    { value: 'danger-full-access', label: 'Danger full access' },
  ];
}

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
  const { t } = useTranslation();
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
        <h3 className="text-sm font-semibold text-rc-text-primary">{t('codexSettings.nativeSettings')}</h3>
        <p className="mt-1 text-xs leading-5 text-rc-text-tertiary">
          {t('codexSettings.nativeSettingsDesc')}
        </p>
      </div>

      <SettingInput
        label="Model provider id"
        description={t('codexSettings.modelProviderDesc')}
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
            {approvalPolicies(t).map((policy) => (
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
            {sandboxModes(t).map((mode) => (
              <option key={mode.value} value={mode.value}>
                {mode.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      <ToggleSwitch
        label={t('codexSettings.saveExtendedHistory')}
        description={t('codexSettings.saveExtendedHistoryDesc')}
        checked={settings.codex_persist_extended_history}
        onChange={(checked) => onUpdate({ codex_persist_extended_history: checked })}
      />

      <ToggleSwitch
        label={t('codexSettings.enableMemories')}
        description={t('codexSettings.enableMemoriesDesc')}
        checked={settings.codex_memories_enabled}
        onChange={(checked) => onUpdate({ codex_memories_enabled: checked })}
      />

      <SettingInput
        label="Thread store endpoint"
        description={t('codexSettings.threadStoreDesc')}
        value={settings.codex_thread_store_endpoint ?? ''}
        onChange={(value) => onUpdate({ codex_thread_store_endpoint: value.trim() || null })}
        placeholder="https://..."
      />

      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-1.5">
          <label className="block text-sm font-medium text-rc-text-primary">Service tier</label>
          <select
            value={settings.codex_service_tier ?? ''}
            onChange={(event) =>
              onUpdate({ codex_service_tier: event.target.value || null })
            }
            className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary focus:border-rc-border-focus focus:outline-none"
          >
            <option value="">Default</option>
            <option value="auto">Auto</option>
            <option value="flex">Flex</option>
            <option value="priority">Priority</option>
          </select>
        </div>

        <ToggleSwitch
          label="Ephemeral"
          description="Don't persist session history to disk"
          checked={settings.codex_ephemeral ?? false}
          onChange={(checked) => onUpdate({ codex_ephemeral: checked })}
        />
      </div>

      <div className="space-y-1.5">
        <div className="flex items-center justify-between gap-3">
          <label className="block text-sm font-medium text-rc-text-primary">
            Config overrides
          </label>
          <span className="text-xs text-rc-text-tertiary">{t('codexSettings.countBadge', { count: overridesCount })}</span>
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
          {t('codexSettings.envHelpText')}
        </p>
      </div>

      <SettingInput
        label="Permission profile"
        description="JSON permission profile passed through to Codex. Leave empty for default."
        value={
          settings.codex_permission_profile
            ? typeof settings.codex_permission_profile === 'string'
              ? settings.codex_permission_profile
              : JSON.stringify(settings.codex_permission_profile)
            : ''
        }
        onChange={(value) => {
          if (!value.trim()) {
            onUpdate({ codex_permission_profile: null });
            return;
          }
          try {
            onUpdate({ codex_permission_profile: JSON.parse(value) });
          } catch {
            onUpdate({ codex_permission_profile: value });
          }
        }}
        placeholder='{"auto_approve": ["read", "search"]}'
      />
    </div>
  );
}
