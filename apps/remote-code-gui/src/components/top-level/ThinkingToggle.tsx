import React from 'react';
import { Brain } from 'lucide-react';
import { cn } from '../../lib/utils';

type Props = {
  currentValue: boolean;
  onSelect: (enabled: boolean) => void;
  onCancel?: () => void;
  isMidConversation?: boolean;
};

export function ThinkingToggle({
  currentValue,
  onSelect,
  onCancel,
  isMidConversation = false,
}: Props): React.ReactElement {
  return (
    <div
      data-testid="thinking-toggle"
      className="rounded-lg border border-gray-200 bg-white p-4 shadow-lg dark:border-gray-700 dark:bg-gray-800"
    >
      <div className="flex items-center gap-2 mb-3">
        <Brain className="h-5 w-5 text-purple-500" />
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          Extended Thinking
        </h3>
      </div>

      {isMidConversation && (
        <p className="mb-3 text-sm text-yellow-600 dark:text-yellow-400">
          Changing thinking mode mid-conversation may affect response quality.
        </p>
      )}

      <div className="flex gap-2">
        <button
          data-testid="thinking-enable"
          className={cn(
            'flex-1 rounded-md border px-4 py-2 text-sm font-medium transition-colors',
            currentValue
              ? 'border-purple-500 bg-purple-50 text-purple-700 dark:border-purple-600 dark:bg-purple-900/20 dark:text-purple-400'
              : 'border-gray-200 text-gray-700 hover:border-gray-300 dark:border-gray-700 dark:text-gray-300',
          )}
          onClick={() => onSelect(true)}
        >
          Enabled
          <p className="mt-0.5 text-xs font-normal opacity-70">
            Claude will think before responding
          </p>
        </button>
        <button
          data-testid="thinking-disable"
          className={cn(
            'flex-1 rounded-md border px-4 py-2 text-sm font-medium transition-colors',
            !currentValue
              ? 'border-purple-500 bg-purple-50 text-purple-700 dark:border-purple-600 dark:bg-purple-900/20 dark:text-purple-400'
              : 'border-gray-200 text-gray-700 hover:border-gray-300 dark:border-gray-700 dark:text-gray-300',
          )}
          onClick={() => onSelect(false)}
        >
          Disabled
          <p className="mt-0.5 text-xs font-normal opacity-70">
            Claude will respond without extended thinking
          </p>
        </button>
      </div>

      {onCancel && (
        <button
          data-testid="thinking-cancel"
          onClick={onCancel}
          className="mt-3 text-sm text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
        >
          Cancel
        </button>
      )}
    </div>
  );
}
