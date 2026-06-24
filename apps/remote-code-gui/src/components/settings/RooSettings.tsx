import { GitBranch, HelpCircle, ListChecks, MessageCircleQuestion, Wrench } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { FullSettings } from '../../lib/types';

export interface RooSettingsProps {
  settings: FullSettings;
  onUpdate: (updates: Record<string, unknown>) => void;
}

type TFn = (key: string, options?: Record<string, unknown>) => string;

function rooModes(t: TFn) {
  return [
    { value: 'code', label: t('chatInput.permission.roo.code'), desc: t('chatInput.permission.roo.codeDesc'), badge: t('rooSettings.modeCodeBadge') },
    { value: 'architect', label: t('chatInput.permission.roo.architect'), desc: t('chatInput.permission.roo.architectDesc'), badge: t('rooSettings.modeArchitectBadge') },
    { value: 'ask', label: t('chatInput.permission.roo.ask'), desc: t('chatInput.permission.roo.askDesc'), badge: t('rooSettings.modeAskBadge') },
    { value: 'debug', label: t('chatInput.permission.roo.debug'), desc: t('chatInput.permission.roo.debugDesc'), badge: t('rooSettings.modeDebugBadge') },
    { value: 'orchestrator', label: t('chatInput.permission.roo.orchestrator'), desc: t('chatInput.permission.roo.orchestratorDesc'), badge: t('rooSettings.modeOrchestratorBadge') },
  ];
}

function interactionCards(t: TFn) {
  return [
    { icon: MessageCircleQuestion, title: t('rooSettings.followupTitle'), desc: t('rooSettings.followupDesc') },
    { icon: ListChecks, title: t('rooSettings.completionTitle'), desc: t('rooSettings.completionDesc') },
    { icon: Wrench, title: t('rooSettings.mistakeTitle'), desc: t('rooSettings.mistakeDesc') },
  ];
}

export function RooSettings({ settings, onUpdate }: RooSettingsProps) {
  const { t } = useTranslation();
  const currentMode = settings.roo_mode ?? 'code';

  return (
    <div className="space-y-5" data-testid="roo-settings">
      <section className="overflow-hidden rounded-md border border-rc-border-secondary bg-rc-bg-surface">
        <div className="border-b border-rc-border-secondary bg-rc-bg-secondary px-4 py-3">
          <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
            <GitBranch size={15} className="text-rc-accent-success" />
            {t('rooSettings.title')}
          </div>
          <p className="mt-1 text-xs leading-5 text-rc-text-tertiary">{t('rooSettings.desc')}</p>
        </div>

        <div className="grid gap-px bg-rc-border-secondary md:grid-cols-5">
          {rooModes(t).map((mode) => {
            const selected = currentMode === mode.value;
            return (
              <button
                key={mode.value}
                type="button"
                onClick={() => onUpdate({ roo_mode: mode.value })}
                className={`bg-rc-bg-surface p-4 text-left transition-colors hover:bg-rc-bg-hover ${
                  selected ? 'ring-1 ring-inset ring-rc-border-focus' : ''
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="text-sm font-semibold text-rc-text-primary">{mode.label}</span>
                  <span className={`rounded px-1.5 py-0.5 text-[10px] font-medium ${
                    selected ? 'bg-rc-bg-selected text-rc-accent-primary' : 'bg-rc-bg-secondary text-rc-text-tertiary'
                  }`}>
                    {mode.badge}
                  </span>
                </div>
                <p className="mt-2 text-xs leading-5 text-rc-text-tertiary">{mode.desc}</p>
              </button>
            );
          })}
        </div>
      </section>

      <section className="grid gap-3 md:grid-cols-3">
        {interactionCards(t).map((card) => {
          const Icon = card.icon;
          return (
            <div key={card.title} className="rounded-md border border-rc-border-secondary bg-rc-bg-surface p-4">
              <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-rc-accent-success">
                <Icon size={14} />
                {card.title}
              </div>
              <p className="mt-2 text-xs leading-5 text-rc-text-tertiary">{card.desc}</p>
            </div>
          );
        })}
      </section>

      <section className="rounded-md border border-rc-border-secondary bg-rc-bg-surface p-4">
        <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
          <HelpCircle size={15} />
          {t('rooSettings.operatorTitle')}
        </div>
        <p className="mt-2 text-xs leading-5 text-rc-text-tertiary">{t('rooSettings.operatorDesc')}</p>
        <div className="mt-3 grid gap-2 sm:grid-cols-3">
          {[t('rooSettings.operatorStep1'), t('rooSettings.operatorStep2'), t('rooSettings.operatorStep3')].map((step, index) => (
            <div key={step} className="rounded-md border border-rc-border-secondary bg-rc-bg-secondary px-3 py-2">
              <div className="text-[10px] font-semibold uppercase text-rc-text-tertiary">{t('rooSettings.stepLabel', { count: index + 1 })}</div>
              <div className="mt-1 text-xs leading-5 text-rc-text-secondary">{step}</div>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
