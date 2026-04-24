import { type ReactNode, useState } from 'react';
import { Clock } from 'lucide-react';
import { WizardDialogLayout, useWizard } from '../../wizard';
import type { ScheduledTaskWizardData } from '../types';

const FREQUENCY_OPTIONS = [
  { label: 'Manual (on demand)', value: 'manual' },
  { label: 'Hourly', value: 'hourly' },
  { label: 'Daily', value: 'daily' },
  { label: 'Weekdays', value: 'weekdays' },
  { label: 'Weekly', value: 'weekly' },
];

export function ScheduleStep(): ReactNode {
  const { goNext, goBack, wizardData, setWizardData } =
    useWizard<ScheduledTaskWizardData>();

  const [showTimePicker, setShowTimePicker] = useState(false);
  const [frequency, setFrequency] = useState(wizardData.frequency ?? 'daily');
  const [time, setTime] = useState(wizardData.scheduledTime ?? '09:00');

  const needsTime = frequency === 'daily' || frequency === 'weekdays' || frequency === 'weekly';

  const handleFrequencySelect = (value: string) => {
    setFrequency(value);
    if (value === 'manual' || value === 'hourly') {
      setWizardData(prev => ({
        ...prev,
        frequency: value,
        scheduledTime: undefined,
        cron: value === 'hourly' ? '0 * * * *' : undefined,
      }));
      goNext();
    } else {
      setShowTimePicker(true);
    }
  };

  const handleTimeSubmit = () => {
    if (!/^\d{1,2}:\d{2}$/.test(time)) return;
    setWizardData(prev => ({
      ...prev,
      frequency,
      scheduledTime: time,
      cron: `${time.split(':')[1]} ${time.split(':')[0]} * * *`,
    }));
    goNext();
  };

  if (showTimePicker && needsTime) {
    return (
      <WizardDialogLayout subtitle="Schedule time">
        <div className="flex flex-col gap-2">
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Enter the time for this task (24-hour format, e.g. 09:00):
          </p>
          <div className="flex items-center gap-2">
            <Clock className="h-4 w-4 text-gray-400" />
            <input
              data-testid="time-input"
              type="text"
              className="rounded border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
              value={time}
              onChange={(e) => setTime(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') handleTimeSubmit(); }}
              placeholder="09:00"
            />
          </div>
          <p className="text-xs text-gray-400 dark:text-gray-500">
            Scheduled tasks use a randomized delay of several minutes for server performance.
          </p>
          <div className="flex gap-2 mt-2">
            <button
              data-testid="time-back"
              onClick={() => setShowTimePicker(false)}
              className="rounded border border-gray-300 px-3 py-1.5 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
            >
              Back
            </button>
            <button
              data-testid="time-submit"
              onClick={handleTimeSubmit}
              className="rounded bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700"
            >
              Next
            </button>
          </div>
        </div>
      </WizardDialogLayout>
    );
  }

  return (
    <WizardDialogLayout subtitle="Frequency">
      <div className="flex flex-col gap-2">
        <p className="text-sm text-gray-500 dark:text-gray-400">
          How often should this task run?
        </p>
        <div className="flex flex-col gap-1" data-testid="frequency-options">
          {FREQUENCY_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              data-testid={`freq-${opt.value}`}
              onClick={() => handleFrequencySelect(opt.value)}
              className={`rounded px-3 py-2 text-left text-sm transition-colors ${
                frequency === opt.value
                  ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                  : 'hover:bg-gray-100 dark:hover:bg-gray-700'
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
        <button
          data-testid="freq-back"
          onClick={goBack}
          className="mt-2 rounded border border-gray-300 px-3 py-1.5 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
        >
          Back
        </button>
      </div>
    </WizardDialogLayout>
  );
}
