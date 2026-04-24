import { type ReactNode, useState } from 'react';
import { WizardDialogLayout, useWizard } from '../../wizard';
import type { ScheduledTaskWizardData } from '../types';

export function TaskPromptStep(): ReactNode {
  const { goNext, goBack, wizardData, setWizardData } =
    useWizard<ScheduledTaskWizardData>();
  const [value, setValue] = useState(wizardData.prompt ?? '');
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = () => {
    const trimmed = value.trim();
    if (!trimmed) {
      setError('Prompt is required');
      return;
    }
    setError(null);
    setWizardData(prev => ({ ...prev, prompt: trimmed }));
    goNext();
  };

  return (
    <WizardDialogLayout subtitle="Prompt">
      <div className="flex flex-col gap-2">
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Enter the prompt that will be sent when this task runs.
        </p>
        <textarea
          data-testid="prompt-input"
          className="min-h-[80px] rounded border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
          value={value}
          onChange={(e) => { setValue(e.target.value); setError(null); }}
          placeholder="e.g. Look at the commits from the last 24 hours..."
        />
        {error && (
          <p data-testid="prompt-error" className="text-sm text-red-500">{error}</p>
        )}
        <div className="flex gap-2 mt-2">
          <button
            data-testid="prompt-back"
            onClick={goBack}
            className="rounded border border-gray-300 px-3 py-1.5 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
          >
            Back
          </button>
          <button
            data-testid="prompt-submit"
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
