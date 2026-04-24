import { type ReactNode } from 'react';
import { Clock, GitBranch, Loader2 } from 'lucide-react';

interface Props {
  id: string;
  title: string;
  messageCount: number;
  modified: string;
  gitBranch?: string;
  isLoading?: boolean;
  onSelect?: () => void;
  onExit?: () => void;
}

export function SessionPreview({
  title,
  messageCount,
  modified,
  gitBranch,
  isLoading = false,
  onSelect,
  onExit,
}: Props): ReactNode {
  if (isLoading) {
    return (
      <div data-testid="session-preview" className="flex items-center gap-2 p-4">
        <Loader2 className="h-5 w-5 animate-spin text-blue-500" />
        <span className="text-sm text-gray-500">Loading session…</span>
        {onExit && (
          <button
            data-testid="session-preview-cancel"
            onClick={onExit}
            className="ml-2 text-xs text-gray-400 hover:text-gray-600"
          >
            Esc to cancel
          </button>
        )}
      </div>
    );
  }

  return (
    <div
      data-testid="session-preview"
      className="flex flex-col rounded border border-gray-200 dark:border-gray-700"
    >
      <div className="flex-1 p-3">
        <h4 className="text-sm font-semibold text-gray-800 dark:text-gray-200">{title}</h4>
      </div>
      <div className="flex items-center gap-3 border-t border-gray-200 px-3 py-2 dark:border-gray-700">
        <div className="flex items-center gap-1 text-xs text-gray-500">
          <Clock className="h-3 w-3" />
          <span>{modified}</span>
        </div>
        <span className="text-xs text-gray-500">
          {messageCount} messages
        </span>
        {gitBranch && (
          <div className="flex items-center gap-1 text-xs text-gray-500">
            <GitBranch className="h-3 w-3" />
            <span>{gitBranch}</span>
          </div>
        )}
        <div className="ml-auto flex gap-2">
          {onExit && (
            <button
              data-testid="session-preview-cancel"
              onClick={onExit}
              className="text-xs text-gray-400 hover:text-gray-600"
            >
              Esc cancel
            </button>
          )}
          {onSelect && (
            <button
              data-testid="session-preview-select"
              onClick={onSelect}
              className="rounded bg-blue-600 px-2 py-0.5 text-xs text-white hover:bg-blue-700"
            >
              Enter resume
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
