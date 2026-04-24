import { type ReactNode } from 'react';
import { Settings, X, AlertCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface ValidationError {
  file: string;
  message: string;
}

interface Props {
  settingsErrors: ValidationError[];
  onContinue: () => void;
  onExit: () => void;
}

export function InvalidSettingsDialog({
  settingsErrors,
  onContinue,
  onExit,
}: Props): ReactNode {
  return (
    <div
      data-testid="invalid-settings-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-amber-200 bg-white p-6 shadow-xl dark:border-amber-800 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Settings className="h-5 w-5 text-amber-500" />
            <h3 className="text-lg font-semibold text-amber-700 dark:text-amber-400">
              Settings Error
            </h3>
          </div>
          <button
            data-testid="invalid-settings-close"
            onClick={onExit}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {settingsErrors.length > 0 && (
          <div className="mt-3 space-y-2">
            {settingsErrors.map((err, index) => (
              <div
                key={index}
                className="flex items-start gap-2 rounded bg-amber-50 p-2 dark:bg-amber-950"
              >
                <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-amber-500" />
                <div>
                  <p className="text-xs font-medium text-amber-700 dark:text-amber-300">{err.file}</p>
                  <p className="text-xs text-amber-600 dark:text-amber-400">{err.message}</p>
                </div>
              </div>
            ))}
          </div>
        )}

        <p className="mt-3 text-xs text-gray-500 dark:text-gray-500">
          Files with errors are skipped entirely, not just the invalid settings.
        </p>

        <div className="mt-4 flex flex-col gap-2">
          <button
            data-testid="invalid-settings-exit"
            onClick={onExit}
            className={cn(
              'rounded px-4 py-2 text-left text-sm',
              'bg-gray-50 text-gray-700 hover:bg-gray-100',
              'dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600',
            )}
          >
            Exit and fix manually
          </button>
          <button
            data-testid="invalid-settings-continue"
            onClick={onContinue}
            className={cn(
              'rounded px-4 py-2 text-left text-sm',
              'bg-amber-50 text-amber-700 hover:bg-amber-100',
              'dark:bg-amber-950 dark:text-amber-300 dark:hover:bg-amber-900',
            )}
          >
            Continue without these settings
          </button>
        </div>
      </div>
    </div>
  );
}
