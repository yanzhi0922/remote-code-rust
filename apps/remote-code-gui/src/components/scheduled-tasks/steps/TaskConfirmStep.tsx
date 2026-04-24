import { type ReactNode } from 'react';
import { CheckCircle } from 'lucide-react';
import { WizardDialogLayout, useWizard } from '../../wizard';
import type { ScheduledTaskWizardData } from '../types';

function formatSchedule(data: ScheduledTaskWizardData): string {
  if (data.cron) return `Cron: ${data.cron}`;
  if (data.frequency === 'manual') return 'Manual (on demand)';
  if (data.frequency) return `${data.frequency}${data.scheduledTime ? ` at ${data.scheduledTime}` : ''}`;
  return 'Not set';
}

export function TaskConfirmStep(): ReactNode {
  const { goNext, goBack, wizardData } =
    useWizard<ScheduledTaskWizardData>();

  const schedule = formatSchedule(wizardData);

  const summaryItems = [
    { label: 'Name', value: wizardData.name ?? '—' },
    { label: 'Description', value: wizardData.description ?? '—' },
    {
      label: 'Prompt',
      value: wizardData.prompt
        ? wizardData.prompt.length > 60
          ? wizardData.prompt.slice(0, 57) + '...'
          : wizardData.prompt
        : '—',
    },
    { label: 'Model', value: wizardData.model ?? 'default' },
    { label: 'Permissions', value: wizardData.permissionMode ?? 'ask' },
    { label: 'Folder', value: wizardData.folder ?? 'current project' },
    { label: 'Worktree', value: wizardData.worktree ? 'yes' : 'no' },
    { label: 'Schedule', value: schedule },
  ];

  return (
    <WizardDialogLayout subtitle="Review & confirm">
      <div className="flex flex-col gap-2">
        <div data-testid="confirm-summary" className="flex flex-col gap-1.5">
          {summaryItems.map((item) => (
            <div key={item.label} className="flex gap-2 text-sm">
              <span className="font-medium text-gray-700 dark:text-gray-300">{item.label}:</span>
              <span className="text-gray-600 dark:text-gray-400">{item.value}</span>
            </div>
          ))}
        </div>
        <p className="mt-2 text-xs text-gray-400 dark:text-gray-500">
          Press Confirm to create the task, or Back to make changes.
        </p>
        <div className="flex gap-2 mt-2">
          <button
            data-testid="confirm-back"
            onClick={goBack}
            className="rounded border border-gray-300 px-3 py-1.5 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
          >
            Back
          </button>
          <button
            data-testid="confirm-submit"
            onClick={goNext}
            className="flex items-center gap-1.5 rounded bg-green-600 px-3 py-1.5 text-sm text-white hover:bg-green-700"
          >
            <CheckCircle className="h-4 w-4" />
            Confirm
          </button>
        </div>
      </div>
    </WizardDialogLayout>
  );
}
