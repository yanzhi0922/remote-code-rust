import { Check, Route, Shield, Sparkles } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ClaudeModelMapping, FullSettings, ProviderConfig } from '../../lib/types';
import { useAppStore } from '../../stores/useAppStore';

export interface ClaudeSettingsProps {
  settings: FullSettings;
  onUpdate: (updates: Record<string, unknown>) => void;
}

type TFn = (key: string) => string;
type ClaudeTier = 'opus' | 'sonnet' | 'haiku';

function permissionModes(t: TFn) {
  return [
    { value: 'default', label: t('chatInput.permission.claude.default'), desc: t('chatInput.permission.claude.defaultDesc') },
    { value: 'acceptEdits', label: t('chatInput.permission.claude.acceptEdits'), desc: t('chatInput.permission.claude.acceptEditsDesc') },
    { value: 'dontAsk', label: t('chatInput.permission.claude.dontAsk'), desc: t('chatInput.permission.claude.dontAskDesc') },
    { value: 'bypassPermissions', label: t('chatInput.permission.claude.bypassPermissions'), desc: t('chatInput.permission.claude.bypassPermissionsDesc') },
    { value: 'plan', label: t('chatInput.permission.claude.plan'), desc: t('chatInput.permission.claude.planDesc') },
  ];
}

function tierLabels(t: TFn): Array<{ tier: ClaudeTier; label: string; desc: string }> {
  return [
    { tier: 'opus', label: t('settings.opusTask'), desc: t('claudeSettings.opusDesc') },
    { tier: 'sonnet', label: t('settings.sonnetTask'), desc: t('claudeSettings.sonnetDesc') },
    { tier: 'haiku', label: t('settings.haikuTask'), desc: t('claudeSettings.haikuDesc') },
  ];
}

function workflowCards(t: TFn) {
  return [
    { title: t('claudeSettings.workflowPlanTitle'), desc: t('claudeSettings.workflowPlanDesc') },
    { title: t('claudeSettings.workflowEditTitle'), desc: t('claudeSettings.workflowEditDesc') },
    { title: t('claudeSettings.workflowReviewTitle'), desc: t('claudeSettings.workflowReviewDesc') },
  ];
}

function modelOptions(provider: ProviderConfig | null) {
  if (!provider) return [];
  const ids = new Set<string>();
  provider.models?.forEach((model) => {
    if (model.id.trim()) ids.add(model.id);
  });
  if (provider.model?.trim()) ids.add(provider.model);
  return Array.from(ids).map((id) => ({ id }));
}

export function ClaudeSettings({ settings, onUpdate }: ClaudeSettingsProps) {
  const { t } = useTranslation();
  const providerConfigs = useAppStore((state) => state.providerConfigs);
  const setClaudeModelMapping = useAppStore((state) => state.setClaudeModelMapping);
  const activeProvider = useMemo(() => {
    const activeName = providerConfigs?.active_provider ?? settings.provider_name;
    return providerConfigs?.providers.find((provider) => provider.name === activeName) ?? null;
  }, [providerConfigs, settings.provider_name]);
  const mapping = activeProvider?.claude_model_mapping ?? {};
  const options = modelOptions(activeProvider);

  const handleTierChange = (tier: ClaudeTier, modelId: string) => {
    if (!activeProvider) return;
    const next: ClaudeModelMapping = { ...mapping, [tier]: modelId || null };
    void setClaudeModelMapping(activeProvider.name, next);
  };

  return (
    <div className="space-y-5" data-testid="claude-settings">
      <section className="overflow-hidden rounded-md border border-rc-border-secondary bg-rc-bg-surface">
        <div className="border-b border-rc-border-secondary bg-rc-bg-secondary px-4 py-3">
          <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
            <Sparkles size={15} className="text-rc-accent-primary" />
            {t('claudeSettings.title')}
          </div>
          <p className="mt-1 text-xs leading-5 text-rc-text-tertiary">{t('claudeSettings.desc')}</p>
        </div>
        <div className="grid gap-px bg-rc-border-secondary md:grid-cols-3">
          {workflowCards(t).map((card) => (
            <div key={card.title} className="bg-rc-bg-surface p-4">
              <div className="text-xs font-semibold uppercase tracking-wide text-rc-accent-primary">{card.title}</div>
              <p className="mt-2 text-xs leading-5 text-rc-text-tertiary">{card.desc}</p>
            </div>
          ))}
        </div>
      </section>

      <section className="space-y-3">
        <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
          <Shield size={15} />
          {t('claudeSettings.permissionTitle')}
        </div>
        <div className="space-y-2">
          {permissionModes(t).map((mode) => (
            <label
              key={mode.value}
              className={`flex cursor-pointer items-start gap-3 rounded-md border px-3 py-2.5 text-sm transition-colors ${
                settings.permission_mode === mode.value
                  ? 'border-rc-border-focus bg-rc-bg-selected'
                  : 'border-rc-border-secondary bg-rc-bg-secondary hover:border-rc-border-hover'
              }`}
            >
              <div className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border border-rc-border-primary">
                {settings.permission_mode === mode.value && (
                  <div className="h-2 w-2 rounded-full bg-rc-accent-primary" />
                )}
              </div>
              <input
                type="radio"
                name="claude_permission_mode"
                value={mode.value}
                checked={settings.permission_mode === mode.value}
                onChange={(event) => onUpdate({ permission_mode: event.target.value })}
                className="sr-only"
              />
              <div>
                <div className="font-medium text-rc-text-primary">{mode.label}</div>
                <div className="mt-1 text-xs leading-5 text-rc-text-tertiary">{mode.desc}</div>
              </div>
            </label>
          ))}
        </div>
      </section>

      <section className="space-y-3 rounded-md border border-rc-border-secondary bg-rc-bg-surface p-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
              <Route size={15} />
              {t('claudeSettings.modelRoutingTitle')}
            </div>
            <p className="mt-1 text-xs leading-5 text-rc-text-tertiary">
              {activeProvider
                ? t('claudeSettings.modelRoutingDesc')
                : t('claudeSettings.noProviderDesc')}
            </p>
          </div>
          <span className="shrink-0 rounded border border-rc-border-primary bg-rc-bg-secondary px-2 py-1 text-[10px] font-medium text-rc-text-tertiary">
            {activeProvider?.name ?? t('settings.unset')}
          </span>
        </div>

        <div className="grid gap-3 md:grid-cols-3">
          {tierLabels(t).map((item) => (
            <label key={item.tier} className="space-y-1.5">
              <span className="block text-xs font-semibold text-rc-text-primary">{item.label}</span>
              <select
                value={mapping[item.tier] ?? ''}
                disabled={!activeProvider || options.length === 0}
                onChange={(event) => handleTierChange(item.tier, event.target.value)}
                className="w-full rounded-md border border-rc-border-primary bg-rc-bg-secondary px-3 py-2 text-sm text-rc-text-primary outline-none focus:border-rc-border-focus disabled:cursor-not-allowed disabled:opacity-50"
                data-testid={`claude-tier-${item.tier}`}
              >
                <option value="">{t('settings.unset')}</option>
                {options.map((option) => (
                  <option key={option.id} value={option.id}>
                    {option.id}
                  </option>
                ))}
              </select>
              <span className="block text-[11px] leading-4 text-rc-text-tertiary">{item.desc}</span>
            </label>
          ))}
        </div>

        <div className="rounded-md border border-rc-border-secondary bg-rc-bg-secondary px-3 py-2 text-xs text-rc-text-tertiary">
          <span className="inline-flex items-center gap-1 font-medium text-rc-text-secondary">
            <Check size={12} />
            {t('claudeSettings.mappingSyncNote')}
          </span>
        </div>
      </section>
    </div>
  );
}
