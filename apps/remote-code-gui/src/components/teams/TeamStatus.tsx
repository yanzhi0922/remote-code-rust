import React from 'react';
import { cn } from '../../lib/utils';

type Props = {
  teamsSelected: boolean;
  showHint: boolean;
  teammateCount: number;
};

export function TeamStatus({
  teamsSelected,
  showHint,
  teammateCount,
}: Props): React.ReactElement | null {
  if (teammateCount === 0) {
    return null;
  }

  const statusText = `${teammateCount} ${teammateCount === 1 ? 'teammate' : 'teammates'}`;

  return (
    <span data-testid="team-status" className="inline-flex items-center">
      <span
        className={cn(
          'rounded px-1.5 py-0.5 text-sm',
          teamsSelected
            ? 'bg-gray-900 text-white dark:bg-gray-100 dark:text-gray-900'
            : 'text-gray-700 dark:text-gray-300',
        )}
      >
        {statusText}
      </span>
      {showHint && teamsSelected && (
        <span className="ml-1 text-sm text-gray-500 dark:text-gray-400">
          · Enter to view
        </span>
      )}
    </span>
  );
}
