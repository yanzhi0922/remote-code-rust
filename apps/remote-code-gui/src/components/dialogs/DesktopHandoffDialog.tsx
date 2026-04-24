import { type ReactNode, useState } from 'react';
import { Monitor, X, Loader2, Download } from 'lucide-react';
import { cn } from '../../lib/utils';

type HandoffState = 'checking' | 'prompt-download' | 'flushing' | 'opening' | 'success' | 'error';

interface Props {
  onDone: (result?: string) => void;
}

export function DesktopHandoffDialog({ onDone }: Props): ReactNode {
  const [state, setState] = useState<HandoffState>('checking');
  const [error] = useState<string | null>(null);
  const [downloadMessage, setDownloadMessage] = useState('');

  const handlePromptDownload = (accept: boolean) => {
    if (accept) {
      setState('success');
      onDone('Starting download. Re-run /desktop once you\'ve installed the app.');
    } else {
      onDone('The desktop app is required for /desktop.');
    }
  };

  const handleStartCheck = () => {
    setState('prompt-download');
    setDownloadMessage('Claude Desktop is not installed.');
  };

  const handleError = () => {
    onDone(error ?? 'Unknown error');
  };

  return (
    <div
      data-testid="desktop-handoff-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Monitor className="h-5 w-5 text-indigo-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Desktop Handoff
            </h3>
          </div>
          <button
            data-testid="desktop-handoff-close"
            onClick={() => onDone()}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {state === 'checking' && (
          <div className="mt-4 flex items-center gap-2">
            <Loader2 className="h-4 w-4 animate-spin text-indigo-500" />
            <p className="text-sm text-gray-600 dark:text-gray-400">
              Checking desktop installation…
            </p>
            <button
              data-testid="desktop-handoff-simulate"
              onClick={handleStartCheck}
              className="text-xs text-blue-600 underline dark:text-blue-400"
            >
              Simulate
            </button>
          </div>
        )}

        {state === 'prompt-download' && (
          <div className="mt-4 space-y-3">
            <p className="text-sm text-gray-600 dark:text-gray-400">{downloadMessage}</p>
            <p className="text-sm text-gray-600 dark:text-gray-400">
              Would you like to download Claude Desktop?
            </p>
            <div className="flex gap-2">
              <button
                data-testid="desktop-handoff-download-yes"
                onClick={() => handlePromptDownload(true)}
                className={cn(
                  'flex items-center gap-1 rounded px-4 py-2 text-sm',
                  'bg-indigo-600 text-white hover:bg-indigo-700',
                )}
              >
                <Download className="h-4 w-4" /> Yes, download
              </button>
              <button
                data-testid="desktop-handoff-download-no"
                onClick={() => handlePromptDownload(false)}
                className="rounded bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300"
              >
                No
              </button>
            </div>
          </div>
        )}

        {state === 'flushing' && (
          <div className="mt-4 flex items-center gap-2">
            <Loader2 className="h-4 w-4 animate-spin text-indigo-500" />
            <p className="text-sm text-gray-600 dark:text-gray-400">Flushing session data…</p>
          </div>
        )}

        {state === 'opening' && (
          <div className="mt-4 flex items-center gap-2">
            <Loader2 className="h-4 w-4 animate-spin text-indigo-500" />
            <p className="text-sm text-gray-600 dark:text-gray-400">Opening Claude Desktop…</p>
          </div>
        )}

        {state === 'success' && (
          <div className="mt-4">
            <p className="text-sm text-green-600 dark:text-green-400">
              Session handed off to Claude Desktop successfully!
            </p>
          </div>
        )}

        {state === 'error' && (
          <div className="mt-4 space-y-2">
            <p className="text-sm text-red-600 dark:text-red-400">
              {error ?? 'Failed to open Claude Desktop'}
            </p>
            <button
              data-testid="desktop-handoff-error-dismiss"
              onClick={handleError}
              className="text-sm text-red-600 underline dark:text-red-400"
            >
              Dismiss
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
