import { type ReactNode, useState } from 'react';
import { Cpu } from 'lucide-react';
import { WizardDialogLayout, useWizard } from '../../wizard';
import type { ScheduledTaskWizardData } from '../types';

const MODEL_OPTIONS = [
  { label: 'Default', value: 'default' },
  { label: 'Claude Sonnet', value: 'claude-sonnet' },
  { label: 'Claude Opus', value: 'claude-opus' },
  { label: 'Claude Haiku', value: 'claude-haiku' },
];

export function TaskModelStep(): ReactNode {
  const { goNext, goBack, wizardData, setWizardData } =
    useWizard<ScheduledTaskWizardData>();
  const [selected, setSelected] = useState(wizardData.model ?? 'default');

  const handleSubmit = () => {
    setWizardData(prev => ({
      ...prev,
      model: selected === 'default' ? undefined : selected,
    }));
    goNext();
  };

  return (
    <WizardDialogLayout subtitle="Model">
      <div className="flex flex-col gap-2">
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Choose the model for this scheduled task.
        </p>
        <div className="flex flex-col gap-1" data-testid="model-options">
          {MODEL_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              data-testid={`model-${opt.value}`}
              onClick={() => setSelected(opt.value)}
              className={`flex items-center gap-2 rounded px-3 py-2 text-left text-sm transition-colors ${
                selected === opt.value
                  ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                  : 'hover:bg-gray-100 dark:hover:bg-gray-700'
              }`}
            >
              <Cpu className="h-4 w-4 text-gray-400" />
              {opt.label}
            </button>
          ))}
        </div>
        <div className="flex gap-2 mt-2">
          <button
            data-testid="model-back"
            onClick={goBack}
            className="rounded border border-gray-300 px-3 py-1.5 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
          >
            Back
          </button>
          <button
            data-testid="model-submit"
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
