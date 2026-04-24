import { type ReactNode } from 'react';
import { Plug, X } from 'lucide-react';
import { cn } from '../../lib/utils';

interface Props {
  onComplete: () => void;
}

export function IdeAutoConnectDialog({ onComplete }: Props): ReactNode {
  const handleYes = () => onComplete();
  const handleNo = () => onComplete();

  return (
    <div
      data-testid="ide-auto-connect-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Plug className="h-5 w-5 text-blue-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Auto-Connect to IDE
            </h3>
          </div>
          <button
            data-testid="ide-auto-connect-close"
            onClick={onComplete}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-3 text-sm text-gray-600 dark:text-gray-400">
          Do you wish to enable auto-connect to IDE?
        </p>
        <p className="mt-1 text-xs text-gray-500 dark:text-gray-500">
          You can also configure this in /config or with the --ide flag
        </p>

        <div className="mt-4 flex justify-end gap-2">
          <button
            data-testid="ide-auto-connect-no"
            onClick={handleNo}
            className={cn(
              'rounded px-4 py-2 text-sm font-medium',
              'bg-gray-100 text-gray-700 hover:bg-gray-200',
              'dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600',
            )}
          >
            No
          </button>
          <button
            data-testid="ide-auto-connect-yes"
            onClick={handleYes}
            className="rounded bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
          >
            Yes
          </button>
        </div>
      </div>
    </div>
  );
}
