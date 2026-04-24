import { type ReactNode } from 'react';
import { AlertTriangle, X } from 'lucide-react';
import { cn } from '../../lib/utils';

interface ChannelEntry {
  kind: 'plugin' | 'server';
  name: string;
  marketplace?: string;
}

interface Props {
  channels: ChannelEntry[];
  onAccept: () => void;
  onExit?: () => void;
}

export function DevChannelsDialog({ channels, onAccept, onExit }: Props): ReactNode {
  const handleExit = () => {
    if (onExit) {
      onExit();
    }
  };

  return (
    <div
      data-testid="dev-channels-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-red-200 bg-white p-6 shadow-xl dark:border-red-800 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <AlertTriangle className="h-5 w-5 text-red-500" />
            <h3 className="text-lg font-semibold text-red-700 dark:text-red-400">
              WARNING: Loading development channels
            </h3>
          </div>
          <button
            data-testid="dev-channels-close"
            onClick={handleExit}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="mt-3 space-y-2">
          <p className="text-sm text-gray-600 dark:text-gray-400">
            --dangerously-load-development-channels is for local channel development only.
            Do not use this option to run channels you have downloaded off the internet.
          </p>
          <p className="text-sm text-gray-600 dark:text-gray-400">
            Please use --channels to run a list of approved channels.
          </p>
          <p className="text-xs text-gray-500 dark:text-gray-500">
            Channels: {channels.map(c =>
              c.kind === 'plugin' ? `plugin:${c.name}@${c.marketplace ?? 'unknown'}` : `server:${c.name}`
            ).join(', ')}
          </p>
        </div>

        <div className="mt-4 flex flex-col gap-2">
          <button
            data-testid="dev-channels-accept"
            onClick={onAccept}
            className={cn(
              'rounded px-4 py-2 text-sm',
              'bg-amber-50 text-amber-700 hover:bg-amber-100',
              'dark:bg-amber-950 dark:text-amber-300 dark:hover:bg-amber-900',
            )}
          >
            I am using this for local development
          </button>
          <button
            data-testid="dev-channels-exit"
            onClick={handleExit}
            className="rounded px-4 py-2 text-sm text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-950"
          >
            Exit
          </button>
        </div>
      </div>
    </div>
  );
}
