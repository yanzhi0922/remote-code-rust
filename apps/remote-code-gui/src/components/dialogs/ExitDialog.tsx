import { type ReactNode } from 'react';
import { LogOut, X } from 'lucide-react';

const GOODBYE_MESSAGES = ['Goodbye!', 'See ya!', 'Bye!', 'Catch you later!'];

function getRandomGoodbyeMessage(): string {
  return GOODBYE_MESSAGES[Math.floor(Math.random() * GOODBYE_MESSAGES.length)];
}

interface Props {
  onDone: (message?: string) => void;
  onCancel?: () => void;
  showWorktree: boolean;
}

export function ExitDialog({ onDone, onCancel, showWorktree }: Props): ReactNode {
  if (!showWorktree) {
    return null;
  }

  const handleExit = () => {
    onDone(getRandomGoodbyeMessage());
  };

  return (
    <div
      data-testid="exit-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <LogOut className="h-5 w-5 text-gray-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Exit Session
            </h3>
          </div>
          {onCancel && (
            <button
              data-testid="exit-dialog-close"
              onClick={onCancel}
              aria-label="Close"
              className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>

        <p className="mt-3 text-sm text-gray-600 dark:text-gray-400">
          Are you sure you want to exit the current session?
        </p>

        <div className="mt-4 flex justify-end gap-2">
          {onCancel && (
            <button
              data-testid="exit-dialog-cancel"
              onClick={onCancel}
              className="rounded bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
            >
              Cancel
            </button>
          )}
          <button
            data-testid="exit-dialog-confirm"
            onClick={handleExit}
            className="rounded bg-red-600 px-4 py-2 text-sm text-white hover:bg-red-700"
          >
            Exit
          </button>
        </div>
      </div>
    </div>
  );
}
