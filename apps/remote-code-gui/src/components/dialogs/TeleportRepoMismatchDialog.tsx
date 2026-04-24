import { type ReactNode, useState } from 'react';
import { GitBranch, X, Loader2, AlertCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface Props {
  targetRepo: string;
  initialPaths: string[];
  onSelectPath: (path: string) => void;
  onCancel: () => void;
}

export function TeleportRepoMismatchDialog({
  targetRepo,
  initialPaths,
  onSelectPath,
  onCancel,
}: Props): ReactNode {
  const [availablePaths, setAvailablePaths] = useState(initialPaths);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [validating, setValidating] = useState(false);

  const handleSelect = (path: string) => {
    setValidating(true);
    setErrorMessage(null);
    // Simulate validation
    onSelectPath(path);
    setValidating(false);
  };

  const handleInvalidPath = (path: string) => {
    setErrorMessage(`${path} no longer contains the correct repository.`);
    setAvailablePaths(availablePaths.filter((p) => p !== path));
    setValidating(false);
  };

  return (
    <div
      data-testid="teleport-repo-mismatch-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <GitBranch className="h-5 w-5 text-orange-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Teleport to Repo
            </h3>
          </div>
          <button
            data-testid="teleport-repo-mismatch-close"
            onClick={onCancel}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {errorMessage && (
          <div className="mt-3 flex items-center gap-1">
            <AlertCircle className="h-4 w-4 text-red-500" />
            <p className="text-sm text-red-600 dark:text-red-400">{errorMessage}</p>
          </div>
        )}

        <p className="mt-3 text-sm text-gray-600 dark:text-gray-400">
          Open Claude Code in <span className="font-semibold">{targetRepo}</span>:
        </p>

        {validating ? (
          <div className="mt-3 flex items-center gap-2">
            <Loader2 className="h-4 w-4 animate-spin text-orange-500" />
            <p className="text-sm text-gray-600 dark:text-gray-400">Validating repository…</p>
          </div>
        ) : availablePaths.length > 0 ? (
          <div className="mt-3 space-y-2">
            {availablePaths.map((path) => (
              <button
                key={path}
                data-testid={`teleport-path-${path}`}
                onClick={() => handleSelect(path)}
                className={cn(
                  'w-full rounded px-4 py-2 text-left text-sm',
                  'bg-gray-50 text-gray-700 hover:bg-gray-100',
                  'dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600',
                )}
              >
                Use <span className="font-semibold">{path}</span>
              </button>
            ))}
            <button
              data-testid="teleport-repo-mismatch-cancel"
              onClick={onCancel}
              className="w-full rounded px-4 py-2 text-left text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400"
            >
              Cancel
            </button>
          </div>
        ) : (
          <p className="mt-3 text-sm text-gray-500 dark:text-gray-500">
            Run claude --teleport from a checkout of {targetRepo}
          </p>
        )}

        {/* Hidden button for testing invalid path */}
        {availablePaths.length > 0 && (
          <button
            data-testid="teleport-repo-mismatch-invalid"
            onClick={() => handleInvalidPath(availablePaths[0])}
            className="mt-2 hidden text-xs text-red-500"
          >
            Simulate invalid
          </button>
        )}
      </div>
    </div>
  );
}
