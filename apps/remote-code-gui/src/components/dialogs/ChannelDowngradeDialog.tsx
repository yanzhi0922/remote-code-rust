import { type ReactNode } from 'react';
import { ArrowDownCircle, X } from 'lucide-react';
import { cn } from '../../lib/utils';

export type ChannelDowngradeChoice = 'downgrade' | 'stay' | 'cancel';

interface Props {
  currentVersion: string;
  onChoice: (choice: ChannelDowngradeChoice) => void;
}

export function ChannelDowngradeDialog({ currentVersion, onChoice }: Props): ReactNode {
  return (
    <div
      data-testid="channel-downgrade-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <ArrowDownCircle className="h-5 w-5 text-amber-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Switch to Stable Channel
            </h3>
          </div>
          <button
            data-testid="channel-downgrade-close"
            onClick={() => onChoice('cancel')}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-3 text-sm text-gray-600 dark:text-gray-400">
          The stable channel may have an older version than what you're currently running ({currentVersion}).
        </p>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-500">
          How would you like to handle this?
        </p>

        <div className="mt-4 flex flex-col gap-2">
          <button
            data-testid="channel-downgrade-allow"
            onClick={() => onChoice('downgrade')}
            className={cn(
              'rounded px-4 py-2 text-left text-sm',
              'bg-amber-50 text-amber-700 hover:bg-amber-100',
              'dark:bg-amber-950 dark:text-amber-300 dark:hover:bg-amber-900',
            )}
          >
            Allow possible downgrade to stable version
          </button>
          <button
            data-testid="channel-downgrade-stay"
            onClick={() => onChoice('stay')}
            className={cn(
              'rounded px-4 py-2 text-left text-sm',
              'bg-gray-50 text-gray-700 hover:bg-gray-100',
              'dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600',
            )}
          >
            Stay on current version ({currentVersion}) until stable catches up
          </button>
          <button
            data-testid="channel-downgrade-cancel"
            onClick={() => onChoice('cancel')}
            className="rounded px-4 py-2 text-left text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
