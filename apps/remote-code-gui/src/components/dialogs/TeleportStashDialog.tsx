import { type ReactNode, useState } from 'react';
import { Archive, X, Loader2, AlertCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface Props {
  onStashAndContinue: () => void;
  onCancel: () => void;
  changedFiles?: string[];
  loading?: boolean;
}

export function TeleportStashDialog({
  onStashAndContinue,
  onCancel,
  changedFiles = [],
  loading = false,
}: Props): ReactNode {
  const [stashing, setStashing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleStash = () => {
    setStashing(true);
    setError(null);
    onStashAndContinue();
  };

  const showFileCount = changedFiles.length > 8;

  return (
    <div
      data-testid="teleport-stash-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Archive className="h-5 w-5 text-amber-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Working Directory Has Changes
            </h3>
          </div>
          <button
            data-testid="teleport-stash-close"
            onClick={onCancel}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-3 text-sm text-gray-600 dark:text-gray-400">
          Teleport will switch git branches. The following changes were found:
        </p>

        <div className="mt-2 rounded bg-gray-50 p-3 dark:bg-gray-900">
          {loading ? (
            <div className="flex items-center gap-2">
              <Loader2 className="h-4 w-4 animate-spin text-gray-500" />
              <p className="text-sm text-gray-500 dark:text-gray-400">Checking git status…</p>
            </div>
          ) : changedFiles.length > 0 ? (
            showFileCount ? (
              <p className="text-sm text-gray-600 dark:text-gray-400">
                {changedFiles.length} files changed
              </p>
            ) : (
              <ul className="space-y-1">
                {changedFiles.map((file, index) => (
                  <li key={index} className="text-xs text-gray-600 dark:text-gray-400">
                    {file}
                  </li>
                ))}
              </ul>
            )
          ) : (
            <p className="text-sm text-gray-500 dark:text-gray-500">No changes detected</p>
          )}
        </div>

        <p className="mt-3 text-sm text-gray-600 dark:text-gray-400">
          Would you like to stash your changes before proceeding?
        </p>

        {error && (
          <div className="mt-2 flex items-center gap-1">
            <AlertCircle className="h-4 w-4 text-red-500" />
            <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
          </div>
        )}

        <div className="mt-4 flex justify-end gap-2">
          <button
            data-testid="teleport-stash-cancel"
            onClick={onCancel}
            className="rounded bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
          >
            Cancel
          </button>
          <button
            data-testid="teleport-stash-confirm"
            onClick={handleStash}
            disabled={stashing}
            className={cn(
              'flex items-center gap-1 rounded px-4 py-2 text-sm text-white',
              stashing
                ? 'bg-gray-400 cursor-not-allowed'
                : 'bg-amber-600 hover:bg-amber-700',
            )}
          >
            {stashing ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" /> Stashing…
              </>
            ) : (
              <>
                <Archive className="h-4 w-4" /> Stash and continue
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
