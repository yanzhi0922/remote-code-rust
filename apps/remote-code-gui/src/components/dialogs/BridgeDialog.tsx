import { type ReactNode, useState } from 'react';
import { Cable, X, QrCode, Copy, CheckCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface Props {
  onDone: () => void;
  connected?: boolean;
  sessionActive?: boolean;
  connectUrl?: string;
  sessionUrl?: string;
  error?: string | null;
  repoName?: string;
  branchName?: string;
}

export function BridgeDialog({
  onDone,
  connected = false,
  sessionActive = false,
  connectUrl,
  sessionUrl,
  error,
  repoName,
  branchName,
}: Props): ReactNode {
  const [showQR, setShowQR] = useState(false);
  const [copied, setCopied] = useState(false);
  const displayUrl = sessionActive ? sessionUrl : connectUrl;

  const handleCopy = () => {
    if (displayUrl) {
      navigator.clipboard.writeText(displayUrl).catch(() => {});
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div
      data-testid="bridge-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-lg rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Cable className="h-5 w-5 text-blue-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              IDE Bridge
            </h3>
          </div>
          <button
            data-testid="bridge-dialog-close"
            onClick={onDone}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {repoName && (
          <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">
            Repository: {repoName}{branchName ? ` (${branchName})` : ''}
          </p>
        )}

        {error && (
          <div className="mt-3 rounded border border-red-200 bg-red-50 p-3 dark:border-red-800 dark:bg-red-950">
            <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
          </div>
        )}

        {connected && (
          <div className="mt-3 flex items-center gap-2">
            <CheckCircle className="h-4 w-4 text-green-500" />
            <span className="text-sm text-green-600 dark:text-green-400">Connected</span>
          </div>
        )}

        {displayUrl && (
          <div className="mt-4 space-y-2">
            <div className="flex items-center gap-2">
              <code className="flex-1 truncate rounded bg-gray-100 px-3 py-2 text-xs dark:bg-gray-900 dark:text-gray-300">
                {displayUrl}
              </code>
              <button
                data-testid="bridge-copy-url"
                onClick={handleCopy}
                className="rounded p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
              >
                {copied ? <CheckCircle className="h-4 w-4 text-green-500" /> : <Copy className="h-4 w-4" />}
              </button>
            </div>
            <button
              data-testid="bridge-toggle-qr"
              onClick={() => setShowQR(!showQR)}
              className={cn(
                'flex items-center gap-1 text-xs',
                'text-blue-600 hover:text-blue-800 dark:text-blue-400',
              )}
            >
              <QrCode className="h-3 w-3" />
              {showQR ? 'Hide QR Code' : 'Show QR Code'}
            </button>
            {showQR && (
              <div data-testid="bridge-qr-code" className="rounded bg-white p-2 dark:bg-gray-100">
                <div className="mx-auto h-32 w-32 bg-gray-200 dark:bg-gray-300" />
              </div>
            )}
          </div>
        )}

        <div className="mt-4 flex justify-end">
          <button
            data-testid="bridge-dialog-done"
            onClick={onDone}
            className="rounded bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
