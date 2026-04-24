import { type ReactNode } from 'react';
import { cn } from '../../lib/utils';

interface Props {
  agentType: string;
  description?: string;
  name?: string;
  toolUseCount: number;
  tokens: number | null;
  isLast: boolean;
  isResolved: boolean;
  isError: boolean;
  isAsync?: boolean;
  lastToolInfo?: string | null;
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export function AgentProgressLine({
  agentType,
  description,
  name,
  toolUseCount,
  tokens,
  isLast,
  isResolved,
  isError,
  isAsync = false,
  lastToolInfo,
}: Props): ReactNode {
  const treeChar = isLast ? '└─' : '├─';
  const isBackgrounded = isAsync && isResolved;

  const getStatusText = (): string => {
    if (!isResolved) return lastToolInfo || 'Initializing…';
    if (isBackgrounded) return 'Running in the background';
    return 'Done';
  };

  return (
    <div data-testid="agent-progress-line" className="flex flex-col">
      <div className="flex items-center gap-1 pl-6">
        <span className="text-gray-400">{treeChar} </span>
        <span
          className={cn(
            'text-sm',
            !isResolved && 'text-gray-500',
            isResolved && !isError && 'text-gray-700 dark:text-gray-300',
            isError && 'text-red-500',
          )}
        >
          <span className="font-semibold">{name ?? description ?? agentType}</span>
          {name && description && (
            <span className="text-gray-500">: {description}</span>
          )}
          {!isBackgrounded && (
            <span className="text-gray-500">
              {' '}&middot; {toolUseCount} tool {toolUseCount === 1 ? 'use' : 'uses'}
              {tokens !== null && <>{' '}&middot; {formatNumber(tokens)} tokens</>}
            </span>
          )}
        </span>
      </div>
      {!isBackgrounded && (
        <div className="flex items-center gap-1 pl-6">
          <span className="text-gray-400">{isLast ? '   ⚿  ' : '│  ⚿  '}</span>
          <span className="text-xs text-gray-500">{getStatusText()}</span>
        </div>
      )}
    </div>
  );
}
