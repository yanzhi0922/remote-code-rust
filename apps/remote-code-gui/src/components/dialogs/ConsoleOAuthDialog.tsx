import { type ReactNode, useState } from 'react';
import { LogIn, X, Loader2, ExternalLink } from 'lucide-react';
import { cn } from '../../lib/utils';

type OAuthStatus = 'idle' | 'ready' | 'waiting' | 'creating' | 'success' | 'error';

interface Props {
  onDone: () => void;
  startingMessage?: string;
  mode?: 'login' | 'setup-token';
}

export function ConsoleOAuthDialog({
  onDone,
  startingMessage,
  mode = 'login',
}: Props): ReactNode {
  const [status, setStatus] = useState<OAuthStatus>('idle');
  const [error, setError] = useState<string | null>(null);

  const handleStart = () => setStatus('waiting');
  const handleCancel = () => onDone();

  return (
    <div
      data-testid="console-oauth-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <LogIn className="h-5 w-5 text-blue-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              {mode === 'setup-token' ? 'Set Up API Token' : 'Sign In'}
            </h3>
          </div>
          <button
            data-testid="console-oauth-close"
            onClick={handleCancel}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {startingMessage && (
          <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">{startingMessage}</p>
        )}

        {status === 'idle' && (
          <div className="mt-4 space-y-3">
            <p className="text-sm text-gray-600 dark:text-gray-400">
              Choose a sign-in method to continue:
            </p>
            <button
              data-testid="console-oauth-claudeai"
              onClick={handleStart}
              className={cn(
                'w-full rounded px-4 py-2 text-left text-sm',
                'bg-blue-50 text-blue-700 hover:bg-blue-100',
                'dark:bg-blue-950 dark:text-blue-300 dark:hover:bg-blue-900',
              )}
            >
              Subscription Plan (Claude Pro/Max)
            </button>
            <button
              data-testid="console-oauth-console"
              onClick={handleStart}
              className={cn(
                'w-full rounded px-4 py-2 text-left text-sm',
                'bg-gray-50 text-gray-700 hover:bg-gray-100',
                'dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600',
              )}
            >
              API Usage Billing (Anthropic Console)
            </button>
          </div>
        )}

        {status === 'waiting' && (
          <div className="mt-4 space-y-3">
            <div className="flex items-center gap-2">
              <Loader2 className="h-4 w-4 animate-spin text-blue-500" />
              <p className="text-sm text-gray-600 dark:text-gray-400">
                Waiting for browser authentication…
              </p>
            </div>
            <p className="text-xs text-gray-500 dark:text-gray-500">
              If your browser didn't open, copy the URL and paste it manually.
            </p>
            <a
              href="#"
              className="inline-flex items-center gap-1 text-xs text-blue-600 dark:text-blue-400"
            >
              Open in browser <ExternalLink className="h-3 w-3" />
            </a>
          </div>
        )}

        {status === 'error' && error && (
          <div className="mt-4 rounded border border-red-200 bg-red-50 p-3 dark:border-red-800 dark:bg-red-950">
            <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
            <button
              data-testid="console-oauth-retry"
              onClick={() => { setStatus('idle'); setError(null); }}
              className="mt-2 text-sm text-red-600 underline dark:text-red-400"
            >
              Retry
            </button>
          </div>
        )}

        <div className="mt-4 flex justify-end">
          <button
            data-testid="console-oauth-cancel"
            onClick={handleCancel}
            className="rounded bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
