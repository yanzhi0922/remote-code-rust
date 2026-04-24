import { type ReactNode, useState } from 'react';
import { WizardDialogLayout, useWizard } from '../../wizard';
import type { ScheduledTaskWizardData } from '../types';

export function TaskDescriptionStep(): ReactNode {
  const { goNext, goBack, wizardData, setWizardData } =
    useWizard<ScheduledTaskWizardData>();
  const [value, setValue] = useState(wizardData.description ?? '');
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = () => {
    const trimmed = value.trim();
    if (!trimmed) {
      setError('Description is required');
      return;
    }
    setError(null);
    setWizardData(prev => ({ ...prev, description: trimmed }));
    goNext();
  };

  return (
    <WizardDialogLayout subtitle="Description">
      <div className="flex flex-col gap-2">
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Briefly describe what this scheduled task does.
        </p>
        <input
          data-testid="description-input"
          type="text"
          className="rounded border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
          value={value}
          onChange={(e) => { setValue(e.target.value); setError(null); }}
          onKeyDown={(e) => { if (e.key === 'Enter') handleSubmit(); }}
          placeholder="e.g. Review yesterday's commits and flag anything concerning"
        />
        {error && (
          <p data-testid="description-error" className="text-sm text-red-500">{error}</p>
        )}
        <div className="flex gap-2 mt-2">
          <button
            data-testid="description-back"
            onClick={goBack}
            className="rounded border border-gray-300 px-3 py-1.5 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
          >
            Back
          </button>
          <button
            data-testid="description-submit"
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
