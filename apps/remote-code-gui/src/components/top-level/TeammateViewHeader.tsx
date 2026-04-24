import { type ReactNode } from 'react';
import { cn } from '../../lib/utils';

interface Props {
  agentName: string;
  color?: string;
  prompt?: string;
  onExit?: () => void;
}

export function TeammateViewHeader({ agentName, color, prompt, onExit }: Props): ReactNode {
  return (
    <div data-testid="teammate-view-header" className="mb-2 flex flex-col">
      <div className="flex items-center gap-1">
        <span className="text-sm text-gray-600 dark:text-gray-400">Viewing</span>
        <span
          className={cn('text-sm font-bold', color ?? 'text-blue-500')}
          data-testid="teammate-name"
        >
          @{agentName}
        </span>
        <span className="text-sm text-gray-400">
          {' '}&middot;{' '}
          <button
            data-testid="teammate-exit"
            onClick={onExit}
            className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            esc to return
          </button>
        </span>
      </div>
      {prompt && (
        <p data-testid="teammate-prompt" className="mt-0.5 text-xs text-gray-500 dark:text-gray-400">
          {prompt}
        </p>
      )}
    </div>
  );
}
