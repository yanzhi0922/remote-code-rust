import React from 'react';
import { AlertTriangle, Info, X } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface StatusNotice {
  id: string;
  type: 'info' | 'warning' | 'error';
  message: string;
  dismissible?: boolean;
}

type Props = {
  notices?: StatusNotice[];
  onDismiss?: (id: string) => void;
};

export function StatusNotices({ notices = [], onDismiss }: Props): React.ReactElement | null {
  if (notices.length === 0) {
    return null;
  }

  const ICONS = {
    info: <Info className="h-4 w-4 text-blue-500" />,
    warning: <AlertTriangle className="h-4 w-4 text-yellow-500" />,
    error: <AlertTriangle className="h-4 w-4 text-red-500" />,
  };

  const COLORS = {
    info: 'bg-blue-50 border-blue-200 dark:bg-blue-900/20 dark:border-blue-700/50',
    warning: 'bg-yellow-50 border-yellow-200 dark:bg-yellow-900/20 dark:border-yellow-700/50',
    error: 'bg-red-50 border-red-200 dark:bg-red-900/20 dark:border-red-700/50',
  };

  const TEXT_COLORS = {
    info: 'text-blue-700 dark:text-blue-400',
    warning: 'text-yellow-700 dark:text-yellow-400',
    error: 'text-red-700 dark:text-red-400',
  };

  return (
    <div data-testid="status-notices" className="flex flex-col gap-2 pl-1">
      {notices.map((notice) => (
        <div
          key={notice.id}
          data-testid={`status-notice-${notice.id}`}
          className={cn(
            'flex items-center justify-between rounded-md border px-3 py-2',
            COLORS[notice.type],
          )}
        >
          <div className="flex items-center gap-2">
            {ICONS[notice.type]}
            <span className={cn('text-sm', TEXT_COLORS[notice.type])}>
              {notice.message}
            </span>
          </div>
          {notice.dismissible && onDismiss && (
            <button
              data-testid={`dismiss-notice-${notice.id}`}
              aria-label={`Dismiss ${notice.id}`}
              onClick={() => onDismiss(notice.id)}
              className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
