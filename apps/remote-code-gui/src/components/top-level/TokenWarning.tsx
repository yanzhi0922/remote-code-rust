import React from 'react';
import { AlertTriangle } from 'lucide-react';
import { cn } from '../../lib/utils';

type WarningLevel = 'normal' | 'warning' | 'critical';

type Props = {
  tokenUsage: number;
  maxTokens: number;
  model?: string;
};

function getWarningLevel(tokenUsage: number, maxTokens: number): WarningLevel {
  const ratio = tokenUsage / maxTokens;
  if (ratio >= 0.9) return 'critical';
  if (ratio >= 0.7) return 'warning';
  return 'normal';
}

export function TokenWarning({ tokenUsage, maxTokens, model }: Props): React.ReactElement | null {
  const level = getWarningLevel(tokenUsage, maxTokens);

  if (level === 'normal') {
    return null;
  }

  const percentage = Math.round((tokenUsage / maxTokens) * 100);

  return (
    <div
      data-testid="token-warning"
      className={cn(
        'flex items-center gap-2 rounded-md px-3 py-2',
        level === 'critical'
          ? 'bg-red-50 dark:bg-red-900/20'
          : 'bg-yellow-50 dark:bg-yellow-900/20',
      )}
    >
      <AlertTriangle
        className={cn(
          'h-4 w-4',
          level === 'critical'
            ? 'text-red-500'
            : 'text-yellow-500',
        )}
      />
      <span
        className={cn(
          'text-sm',
          level === 'critical'
            ? 'text-red-700 dark:text-red-400'
            : 'text-yellow-700 dark:text-yellow-400',
        )}
      >
        {level === 'critical'
          ? `Token usage critical: ${percentage}% used (${formatTokens(tokenUsage)} / ${formatTokens(maxTokens)})`
          : `Token usage high: ${percentage}% used (${formatTokens(tokenUsage)} / ${formatTokens(maxTokens)})`}
      </span>
      {model && (
        <span className="text-xs text-gray-500 dark:text-gray-400">
          ({model})
        </span>
      )}
    </div>
  );
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}K`;
  return n.toString();
}

export { getWarningLevel, formatTokens };
