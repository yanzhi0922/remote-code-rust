import React from 'react';
import { cn } from '../../lib/utils';

type OverrideMode = 'open' | 'closed';

type Props = {
  isEnabled: boolean;
  isLocked: boolean;
  currentAllowUnsandboxed: boolean;
  onModeChange?: (mode: OverrideMode) => void;
};

export function SandboxOverridesTab({
  isEnabled,
  isLocked,
  currentAllowUnsandboxed,
  onModeChange,
}: Props): React.ReactElement {
  if (!isEnabled) {
    return (
      <div data-testid="sandbox-overrides-tab" className="flex flex-col py-2">
        <span className="text-gray-500 dark:text-gray-400">
          Sandbox is not enabled. Enable sandbox to configure override settings.
        </span>
      </div>
    );
  }

  if (isLocked) {
    return (
      <div data-testid="sandbox-overrides-tab" className="flex flex-col py-2">
        <span className="text-gray-500 dark:text-gray-400">
          Override settings are managed by a higher-priority configuration and cannot be changed
          locally.
        </span>
        <div className="mt-2">
          <span className="text-sm text-gray-500 dark:text-gray-400">
            Current setting:{' '}
            {currentAllowUnsandboxed ? 'Allow unsandboxed fallback' : 'Strict sandbox mode'}
          </span>
        </div>
      </div>
    );
  }

  const currentMode: OverrideMode = currentAllowUnsandboxed ? 'open' : 'closed';

  return (
    <div data-testid="sandbox-overrides-tab" className="flex flex-col py-2">
      <div className="flex flex-col gap-2">
        <button
          data-testid="override-mode-open"
          className={cn(
            'rounded-md border px-3 py-2 text-left text-sm transition-colors',
            currentMode === 'open'
              ? 'border-green-500 bg-green-50 text-green-700 dark:border-green-600 dark:bg-green-900/20 dark:text-green-400'
              : 'border-gray-200 text-gray-700 hover:border-gray-300 dark:border-gray-700 dark:text-gray-300 dark:hover:border-gray-600',
          )}
          onClick={() => onModeChange?.('open')}
        >
          <span className="font-medium">Allow unsandboxed fallback</span>
          {currentMode === 'open' && (
            <span className="ml-2 text-xs text-green-600 dark:text-green-400">(current)</span>
          )}
        </button>
        <button
          data-testid="override-mode-closed"
          className={cn(
            'rounded-md border px-3 py-2 text-left text-sm transition-colors',
            currentMode === 'closed'
              ? 'border-green-500 bg-green-50 text-green-700 dark:border-green-600 dark:bg-green-900/20 dark:text-green-400'
              : 'border-gray-200 text-gray-700 hover:border-gray-300 dark:border-gray-700 dark:text-gray-300 dark:hover:border-gray-600',
          )}
          onClick={() => onModeChange?.('closed')}
        >
          <span className="font-medium">Strict sandbox mode</span>
          {currentMode === 'closed' && (
            <span className="ml-2 text-xs text-green-600 dark:text-green-400">(current)</span>
          )}
        </button>
      </div>
    </div>
  );
}
