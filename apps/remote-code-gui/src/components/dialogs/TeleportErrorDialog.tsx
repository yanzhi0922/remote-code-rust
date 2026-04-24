import { type ReactNode, useState } from 'react';
import { AlertTriangle, X, LogIn, Archive } from 'lucide-react';
import { cn } from '../../lib/utils';

export type TeleportLocalErrorType = 'needsLogin' | 'needsGitStash';

interface Props {
  onComplete: () => void;
  onCancel: () => void;
  errorType?: TeleportLocalErrorType | null;
}

export function TeleportErrorDialog({ onComplete, onCancel, errorType }: Props): ReactNode {
  const [isLoggingIn, setIsLoggingIn] = useState(false);

  if (!errorType) {
    return null;
  }

  const handleLogin = () => {
    setIsLoggingIn(true);
    onComplete();
  };

  return (
    <div
      data-testid="teleport-error-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-red-200 bg-white p-6 shadow-xl dark:border-red-800 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <AlertTriangle className="h-5 w-5 text-red-500" />
            <h3 className="text-lg font-semibold text-red-700 dark:text-red-400">
              Teleport Error
            </h3>
          </div>
          <button
            data-testid="teleport-error-close"
            onClick={onCancel}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {errorType === 'needsLogin' && !isLoggingIn && (
          <div className="mt-4 space-y-3">
            <p className="text-sm text-gray-600 dark:text-gray-400">
              You need to log in before using teleport.
            </p>
            <div className="flex gap-2">
              <button
                data-testid="teleport-error-login"
                onClick={handleLogin}
                className={cn(
                  'flex items-center gap-1 rounded px-4 py-2 text-sm',
                  'bg-blue-600 text-white hover:bg-blue-700',
                )}
              >
                <LogIn className="h-4 w-4" /> Log in
              </button>
              <button
                data-testid="teleport-error-cancel-login"
                onClick={onCancel}
                className="rounded bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300"
              >
                Cancel
              </button>
            </div>
          </div>
        )}

        {errorType === 'needsLogin' && isLoggingIn && (
          <div className="mt-4">
            <p className="text-sm text-gray-600 dark:text-gray-400">
              Logging in…
            </p>
          </div>
        )}

        {errorType === 'needsGitStash' && (
          <div className="mt-4 space-y-3">
            <p className="text-sm text-gray-600 dark:text-gray-400">
              You have uncommitted changes that need to be stashed before teleporting.
            </p>
            <div className="flex gap-2">
              <button
                data-testid="teleport-error-stash"
                onClick={onComplete}
                className={cn(
                  'flex items-center gap-1 rounded px-4 py-2 text-sm',
                  'bg-amber-600 text-white hover:bg-amber-700',
                )}
              >
                <Archive className="h-4 w-4" /> Stash and continue
              </button>
              <button
                data-testid="teleport-error-cancel-stash"
                onClick={onCancel}
                className="rounded bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300"
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
