import { type ReactNode } from 'react';
import { KeyRound, X } from 'lucide-react';
import { cn } from '../../lib/utils';

interface Props {
  customApiKeyTruncated: string;
  onDone: (approved: boolean) => void;
}

export function ApproveApiKeyDialog({ customApiKeyTruncated, onDone }: Props): ReactNode {
  return (
    <div
      data-testid="approve-api-key-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <KeyRound className="h-5 w-5 text-amber-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Approve API Key
            </h3>
          </div>
          <button
            data-testid="approve-api-key-close"
            onClick={() => onDone(false)}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="mt-4">
          <p className="text-sm text-gray-600 dark:text-gray-400">
            <span className="font-semibold">ANTHROPIC_API_KEY</span>: sk-ant-...{customApiKeyTruncated}
          </p>
          <p className="mt-2 text-sm text-gray-600 dark:text-gray-400">
            Do you want to use this API key?
          </p>
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <button
            data-testid="approve-api-key-no"
            onClick={() => onDone(false)}
            className={cn(
              'rounded px-4 py-2 text-sm font-medium',
              'bg-gray-100 text-gray-700 hover:bg-gray-200',
              'dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600',
            )}
          >
            No (recommended)
          </button>
          <button
            data-testid="approve-api-key-yes"
            onClick={() => onDone(true)}
            className={cn(
              'rounded px-4 py-2 text-sm font-medium',
              'bg-blue-600 text-white hover:bg-blue-700',
            )}
          >
            Yes
          </button>
        </div>
      </div>
    </div>
  );
}
