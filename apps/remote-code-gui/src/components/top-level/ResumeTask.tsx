import { type ReactNode, useState } from 'react';
import { RotateCcw, Loader2, AlertCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface SessionItem {
  id: string;
  title: string;
  updated_at: string;
}

interface Props {
  sessions?: SessionItem[];
  onSelect: (session: SessionItem) => void;
  onCancel: () => void;
  loading?: boolean;
  error?: string | null;
}

export function ResumeTask({ sessions = [], onSelect, onCancel, loading = false, error }: Props): ReactNode {
  const [focusedIndex, setFocusedIndex] = useState(0);

  if (loading) {
    return (
      <div data-testid="resume-task" className="flex flex-col items-center gap-2 p-4">
        <Loader2 className="h-5 w-5 animate-spin text-blue-500" />
        <span className="text-sm font-semibold text-gray-700 dark:text-gray-300">Loading sessions…</span>
      </div>
    );
  }

  if (error) {
    return (
      <div data-testid="resume-task" className="flex flex-col gap-2 p-4">
        <div className="flex items-center gap-2 text-red-500">
          <AlertCircle className="h-5 w-5" />
          <span className="text-sm font-semibold">Error loading sessions</span>
        </div>
        <p className="text-sm text-gray-600 dark:text-gray-400">{error}</p>
        <button
          data-testid="resume-cancel"
          onClick={onCancel}
          className="text-sm text-gray-500 hover:text-gray-700"
        >
          Press Esc to cancel
        </button>
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <div data-testid="resume-task" className="flex flex-col gap-2 p-4">
        <div className="flex items-center gap-2">
          <RotateCcw className="h-5 w-5 text-gray-400" />
          <span className="text-sm font-semibold text-gray-700 dark:text-gray-300">No sessions found</span>
        </div>
        <button
          data-testid="resume-cancel-empty"
          onClick={onCancel}
          className="text-sm text-gray-500 hover:text-gray-700"
        >
          Press Esc to cancel
        </button>
      </div>
    );
  }

  return (
    <div data-testid="resume-task" className="flex flex-col gap-1 p-2">
      <h4 className="mb-1 text-sm font-semibold text-gray-700 dark:text-gray-300">
        Select a session to resume ({focusedIndex + 1} of {sessions.length}):
      </h4>
      <div className="text-xs font-semibold text-gray-500 dark:text-gray-400 ml-4 mb-1">
        {'Updated'.padEnd(12)}{'  '}Session Title
      </div>
      <div className="flex flex-col gap-0.5">
        {sessions.map((session, i) => (
          <button
            key={session.id}
            data-testid={`resume-session-${session.id}`}
            onClick={() => { setFocusedIndex(i); onSelect(session); }}
            onMouseEnter={() => setFocusedIndex(i)}
            className={cn(
              'flex items-center gap-2 rounded px-3 py-1.5 text-left text-sm',
              i === focusedIndex
                ? 'bg-blue-50 dark:bg-blue-900/30'
                : 'hover:bg-gray-50 dark:hover:bg-gray-700/50',
            )}
          >
            <span className="w-24 shrink-0 text-xs text-gray-500">{session.updated_at}</span>
            <span className="truncate text-gray-800 dark:text-gray-200">{session.title}</span>
          </button>
        ))}
      </div>
      <button
        data-testid="resume-cancel"
        onClick={onCancel}
        className="mt-2 text-sm text-gray-500 hover:text-gray-700"
      >
        Press Esc to cancel
      </button>
    </div>
  );
}
