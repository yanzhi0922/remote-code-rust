import { type ReactNode, useState } from 'react';
import { GitBranch, X, Loader2, AlertCircle, Trash2, Archive } from 'lucide-react';
import { cn } from '../../lib/utils';

interface Props {
  onDone: (result?: string) => void;
  onCancel?: () => void;
  changes?: string[];
  commitCount?: number;
}

export function WorktreeExitDialog({
  onDone,
  onCancel,
  changes = [],
  commitCount = 0,
}: Props): ReactNode {
  const [status, setStatus] = useState<'loading' | 'asking' | 'removing' | 'done'>('asking');
  const [resultMessage, setResultMessage] = useState<string | undefined>();

  const handleKeep = () => {
    setStatus('done');
    setResultMessage('Worktree kept');
    onDone('Worktree kept');
  };

  const handleRemove = () => {
    setStatus('removing');
    setTimeout(() => {
      setStatus('done');
      setResultMessage('Worktree removed');
      onDone('Worktree removed');
    }, 500);
  };

  const handleKeepWithTmux = () => {
    setStatus('done');
    setResultMessage('Worktree kept, tmux session preserved');
    onDone('Worktree kept with tmux');
  };

  if (status === 'loading' || status === 'done') {
    return (
      <div
        data-testid="worktree-exit-dialog"
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      >
        <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
          <div className="flex items-center gap-2">
            {status === 'loading' ? (
              <Loader2 className="h-5 w-5 animate-spin text-gray-500" />
            ) : (
              <AlertCircle className="h-5 w-5 text-green-500" />
            )}
            <p className="text-sm text-gray-600 dark:text-gray-400">
              {status === 'loading' ? 'Loading worktree status…' : resultMessage}
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      data-testid="worktree-exit-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <GitBranch className="h-5 w-5 text-orange-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Exit Worktree
            </h3>
          </div>
          {onCancel && (
            <button
              data-testid="worktree-exit-close"
              onClick={onCancel}
              aria-label="Close"
              className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>

        {changes.length > 0 && (
          <div className="mt-3">
            <p className="text-sm text-gray-600 dark:text-gray-400">
              Uncommitted changes ({changes.length} files):
            </p>
            <ul className="mt-1 space-y-0.5">
              {changes.slice(0, 8).map((file, index) => (
                <li key={index} className="text-xs text-gray-500 dark:text-gray-400">{file}</li>
              ))}
              {changes.length > 8 && (
                <li className="text-xs text-gray-400 dark:text-gray-500">
                  …and {changes.length - 8} more
                </li>
              )}
            </ul>
          </div>
        )}

        {commitCount > 0 && (
          <p className="mt-2 text-sm text-gray-600 dark:text-gray-400">
            {commitCount} commit{commitCount !== 1 ? 's' : ''} to eject
          </p>
        )}

        <div className="mt-4 flex flex-col gap-2">
          <button
            data-testid="worktree-exit-keep"
            onClick={handleKeep}
            className={cn(
              'flex items-center gap-1 rounded px-4 py-2 text-left text-sm',
              'bg-gray-50 text-gray-700 hover:bg-gray-100',
              'dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600',
            )}
          >
            <Archive className="h-4 w-4" /> Keep worktree
          </button>
          <button
            data-testid="worktree-exit-keep-tmux"
            onClick={handleKeepWithTmux}
            className={cn(
              'flex items-center gap-1 rounded px-4 py-2 text-left text-sm',
              'bg-blue-50 text-blue-700 hover:bg-blue-100',
              'dark:bg-blue-950 dark:text-blue-300 dark:hover:bg-blue-900',
            )}
          >
            Keep worktree and tmux session
          </button>
          <button
            data-testid="worktree-exit-remove"
            onClick={handleRemove}
            className={cn(
              'flex items-center gap-1 rounded px-4 py-2 text-left text-sm',
              'text-red-600 hover:bg-red-50',
              'dark:text-red-400 dark:hover:bg-red-950',
            )}
          >
            <Trash2 className="h-4 w-4" /> Remove worktree
          </button>
        </div>
      </div>
    </div>
  );
}
