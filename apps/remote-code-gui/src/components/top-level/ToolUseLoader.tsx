import { type ReactNode } from 'react';
import { cn } from '../../lib/utils';

interface Props {
  isError: boolean;
  isUnresolved: boolean;
  shouldAnimate: boolean;
}

export function ToolUseLoader({ isError, isUnresolved, shouldAnimate }: Props): ReactNode {
  const color = isUnresolved
    ? undefined
    : isError
      ? 'text-red-500'
      : 'text-green-500';

  return (
    <div
      data-testid="tool-use-loader"
      className={cn(
        'flex h-5 w-5 items-center justify-center text-sm',
        shouldAnimate && 'animate-pulse',
        color,
        isUnresolved && 'text-gray-400',
      )}
    >
      ●
    </div>
  );
}
