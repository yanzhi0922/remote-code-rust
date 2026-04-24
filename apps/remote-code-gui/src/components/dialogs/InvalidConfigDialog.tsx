import { type ReactNode } from 'react';
import { AlertTriangle, X } from 'lucide-react';
import { cn } from '../../lib/utils';

interface Props {
  filePath: string;
  errorDescription: string;
  onExit: () => void;
  onReset: () => void;
}

export function InvalidConfigDialog({
  filePath,
  errorDescription,
  onExit,
  onReset,
}: Props): ReactNode {
  return (
    <div
      data-testid="invalid-config-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-red-200 bg-white p-6 shadow-xl dark:border-red-800 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <AlertTriangle className="h-5 w-5 text-red-500" />
            <h3 className="text-lg font-semibold text-red-700 dark:text-red-400">
              Invalid Configuration
            </h3>
          </div>
          <button
            data-testid="invalid-config-close"
            onClick={onExit}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="mt-3 space-y-2">
          <p className="text-sm text-gray-600 dark:text-gray-400">
            The configuration file at <span className="font-semibold">{filePath}</span> contains invalid JSON.
          </p>
          <p className="text-sm text-red-600 dark:text-red-400">{errorDescription}</p>
        </div>

        <p className="mt-3 text-sm font-semibold text-gray-700 dark:text-gray-300">
          Choose an option:
        </p>

        <div className="mt-2 flex flex-col gap-2">
          <button
            data-testid="invalid-config-exit"
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
            data-testid="invalid-config-reset"
            onClick={onReset}
            className={cn(
              'rounded px-4 py-2 text-left text-sm',
              'bg-amber-50 text-amber-700 hover:bg-amber-100',
              'dark:bg-amber-950 dark:text-amber-300 dark:hover:bg-amber-900',
            )}
          >
            Reset with default configuration
          </button>
        </div>
      </div>
    </div>
  );
}
