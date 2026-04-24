import { type ReactNode } from 'react';
import { Zap, CheckCircle, Circle, Loader2 } from 'lucide-react';
import { cn } from '../../lib/utils';

export type TeleportProgressStep = 'validating' | 'fetching_logs' | 'fetching_branch' | 'checking_out';

interface StepInfo {
  key: TeleportProgressStep;
  label: string;
}

const STEPS: StepInfo[] = [
  { key: 'validating', label: 'Validating session' },
  { key: 'fetching_logs', label: 'Fetching session logs' },
  { key: 'fetching_branch', label: 'Getting branch info' },
  { key: 'checking_out', label: 'Checking out branch' },
];

interface Props {
  currentStep: TeleportProgressStep;
  sessionId?: string;
}

export function TeleportProgressDialog({ currentStep, sessionId }: Props): ReactNode {
  const currentStepIndex = STEPS.findIndex((s) => s.key === currentStep);

  return (
    <div
      data-testid="teleport-progress-dialog"
      className="rounded-lg border border-blue-200 bg-white p-4 shadow-lg dark:border-blue-800 dark:bg-gray-800"
    >
      <div className="flex items-center gap-2">
        <Zap className="h-5 w-5 text-blue-500" />
        <h4 className="font-semibold text-blue-700 dark:text-blue-400">
          Teleporting session…
        </h4>
      </div>

      {sessionId && (
        <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">{sessionId}</p>
      )}

      <div className="mt-3 space-y-2">
        {STEPS.map((step, index) => {
          const isComplete = index < currentStepIndex;
          const isCurrent = index === currentStepIndex;
          const isPending = index > currentStepIndex;

          return (
            <div key={step.key} className="flex items-center gap-2">
              {isComplete && (
                <CheckCircle className="h-4 w-4 text-green-500" />
              )}
              {isCurrent && (
                <Loader2 className="h-4 w-4 animate-spin text-blue-500" />
              )}
              {isPending && (
                <Circle className={cn('h-4 w-4', 'text-gray-300 dark:text-gray-600')} />
              )}
              <span
                className={cn(
                  'text-sm',
                  isComplete && 'text-green-600 dark:text-green-400',
                  isCurrent && 'font-semibold text-blue-600 dark:text-blue-400',
                  isPending && 'text-gray-400 dark:text-gray-500',
                )}
              >
                {step.label}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
