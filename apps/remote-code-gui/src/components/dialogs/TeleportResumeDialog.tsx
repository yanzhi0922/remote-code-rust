import { type ReactNode, useState } from 'react';
import { RotateCcw, X, Loader2, AlertCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface SessionInfo {
  id: string;
  title?: string;
  createdAt?: string;
}

interface Props {
  onComplete: (sessionId: string) => void;
  onCancel: () => void;
  onError?: (error: string) => void;
  sessions?: SessionInfo[];
  loading?: boolean;
  error?: string | null;
}

export function TeleportResumeDialog({
  onComplete,
  onCancel,
  onError,
  sessions = [],
  loading = false,
  error,
}: Props): ReactNode {
  const [selectedSession, setSelectedSession] = useState<string | null>(null);

  const handleSelect = (sessionId: string) => {
    setSelectedSession(sessionId);
  };

  const handleResume = () => {
    if (selectedSession) {
      onComplete(selectedSession);
    }
  };

  const handleError = () => {
    if (error && onError) {
      onError(error);
    }
  };

  return (
    <div
      data-testid="teleport-resume-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <RotateCcw className="h-5 w-5 text-teal-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Resume Session
            </h3>
          </div>
          <button
            data-testid="teleport-resume-close"
            onClick={onCancel}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {error && (
          <div className="mt-3 flex items-center gap-1">
            <AlertCircle className="h-4 w-4 text-red-500" />
            <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
            <button
              data-testid="teleport-resume-error-dismiss"
              onClick={handleError}
              className="text-xs text-red-500 underline"
            >
              Dismiss
            </button>
          </div>
        )}

        {loading ? (
          <div className="mt-4 flex items-center gap-2">
            <Loader2 className="h-4 w-4 animate-spin text-teal-500" />
            <p className="text-sm text-gray-600 dark:text-gray-400">Loading sessions…</p>
          </div>
        ) : (
          <div className="mt-4 space-y-2">
            {sessions.length === 0 ? (
              <p className="text-sm text-gray-500 dark:text-gray-500">
                No sessions available for resume.
              </p>
            ) : (
              sessions.map((session) => (
                <button
                  key={session.id}
                  data-testid={`teleport-resume-session-${session.id}`}
                  onClick={() => handleSelect(session.id)}
                  className={cn(
                    'w-full rounded px-4 py-2 text-left text-sm',
                    selectedSession === session.id
                      ? 'border-2 border-teal-500 bg-teal-50 dark:bg-teal-950'
                      : 'border border-gray-200 bg-gray-50 hover:bg-gray-100 dark:border-gray-600 dark:bg-gray-700 dark:hover:bg-gray-600',
                  )}
                >
                  <p className="font-medium text-gray-900 dark:text-gray-100">
                    {session.title ?? session.id}
                  </p>
                  {session.createdAt && (
                    <p className="text-xs text-gray-500 dark:text-gray-400">{session.createdAt}</p>
                  )}
                </button>
              ))
            )}
          </div>
        )}

        <div className="mt-4 flex justify-end gap-2">
          <button
            data-testid="teleport-resume-cancel"
            onClick={onCancel}
            className="rounded bg-gray-100 px-4 py-2 text-sm text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
          >
            Cancel
          </button>
          <button
            data-testid="teleport-resume-confirm"
            onClick={handleResume}
            disabled={!selectedSession}
            className={cn(
              'rounded px-4 py-2 text-sm text-white',
              selectedSession
                ? 'bg-teal-600 hover:bg-teal-700'
                : 'bg-gray-400 cursor-not-allowed',
            )}
          >
            Resume
          </button>
        </div>
      </div>
    </div>
  );
}
