import { type ReactNode, useState } from 'react';
import { Shield, ShieldCheck, ShieldAlert, ShieldOff } from 'lucide-react';
import { WizardDialogLayout, useWizard } from '../../wizard';
import type { ScheduledTaskWizardData } from '../types';

const PERMISSION_OPTIONS = [
  {
    label: 'Ask permissions',
    value: 'ask',
    description: 'Always ask before making changes',
    icon: Shield,
  },
  {
    label: 'Auto accept edits',
    value: 'auto-accept',
    description: 'Automatically accept all file edits',
    icon: ShieldCheck,
  },
  {
    label: 'Plan mode',
    value: 'plan',
    description: 'Create a plan before making changes',
    icon: ShieldAlert,
  },
  {
    label: 'Bypass permissions',
    value: 'bypass',
    description: 'Accepts all permissions',
    icon: ShieldOff,
  },
];

export function PermissionStep(): ReactNode {
  const { goNext, goBack, wizardData, setWizardData } =
    useWizard<ScheduledTaskWizardData>();
  const [selected, setSelected] = useState(wizardData.permissionMode ?? 'ask');

  const handleSubmit = () => {
    setWizardData(prev => ({ ...prev, permissionMode: selected }));
    goNext();
  };

  return (
    <WizardDialogLayout subtitle="Permission mode">
      <div className="flex flex-col gap-2">
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Choose the permission mode for this scheduled task.
        </p>
        <div className="flex flex-col gap-1" data-testid="permission-options">
          {PERMISSION_OPTIONS.map((opt) => {
            const Icon = opt.icon;
            return (
              <button
                key={opt.value}
                data-testid={`perm-${opt.value}`}
                onClick={() => setSelected(opt.value)}
                className={`flex items-start gap-2 rounded px-3 py-2 text-left text-sm transition-colors ${
                  selected === opt.value
                    ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                    : 'hover:bg-gray-100 dark:hover:bg-gray-700'
                }`}
              >
                <Icon className="mt-0.5 h-4 w-4 shrink-0 text-gray-400" />
                <div>
                  <div className="font-medium">{opt.label}</div>
                  <div className="text-xs text-gray-500 dark:text-gray-400">{opt.description}</div>
                </div>
              </button>
            );
          })}
        </div>
        <div className="flex gap-2 mt-2">
          <button
            data-testid="perm-back"
            onClick={goBack}
            className="rounded border border-gray-300 px-3 py-1.5 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
          >
            Back
          </button>
          <button
            data-testid="perm-submit"
            onClick={handleSubmit}
            className="rounded bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700"
          >
            Next
          </button>
        </div>
      </div>
    </WizardDialogLayout>
  );
}
