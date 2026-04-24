import { type ReactNode } from 'react';
import { Clock, X } from 'lucide-react';
import { cn } from '../../lib/utils';

type IdleReturnAction = 'continue' | 'clear' | 'dismiss' | 'never';

interface Props {
  idleMinutes: number;
  totalInputTokens: number;
  onDone: (action: IdleReturnAction) => void;
}

function formatIdleDuration(minutes: number): string {
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
}

export function IdleReturnDialog({ idleMinutes, totalInputTokens, onDone }: Props): ReactNode {
  const formattedIdle = formatIdleDuration(idleMinutes);
  const formattedTokens = totalInputTokens.toLocaleString();

  return (
    <div
      data-testid="idle-return-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Clock className="h-5 w-5 text-amber-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              You've been away {formattedIdle}
            </h3>
          </div>
          <button
            data-testid="idle-return-close"
            onClick={() => onDone('dismiss')}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-2 text-sm text-gray-600 dark:text-gray-400">
          This conversation is {formattedTokens} tokens.
        </p>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-500">
          If this is a new task, clearing context will save usage and be faster.
        </p>

        <div className="mt-4 flex flex-col gap-2">
          <button
            data-testid="idle-return-continue"
            onClick={() => onDone('continue')}
            className={cn(
              'rounded px-4 py-2 text-left text-sm',
              'bg-blue-50 text-blue-700 hover:bg-blue-100',
              'dark:bg-blue-950 dark:text-blue-300 dark:hover:bg-blue-900',
            )}
          >
            Continue this conversation
          </button>
          <button
            data-testid="idle-return-clear"
            onClick={() => onDone('clear')}
            className={cn(
              'rounded px-4 py-2 text-left text-sm',
              'bg-gray-50 text-gray-700 hover:bg-gray-100',
              'dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600',
            )}
          >
            Send message as a new conversation
          </button>
          <button
            data-testid="idle-return-never"
            onClick={() => onDone('never')}
            className="rounded px-4 py-2 text-left text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400"
          >
            Don't ask me again
          </button>
        </div>
      </div>
    </div>
  );
}
