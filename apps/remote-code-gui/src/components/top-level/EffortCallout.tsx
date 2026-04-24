import { type ReactNode } from 'react';
import { cn } from '../../lib/utils';

type EffortLevel = 'low' | 'medium' | 'high';

interface Props {
  level: EffortLevel;
  onDone?: (level: EffortLevel | undefined) => void;
}

const EFFORT_CONFIG: Record<EffortLevel, { symbol: string; label: string; color: string; description: string }> = {
  low: { symbol: '○', label: 'Low', color: 'text-blue-400', description: 'Quick responses, less thinking' },
  medium: { symbol: '◐', label: 'Medium (recommended)', color: 'text-yellow-400', description: 'Balanced speed and quality' },
  high: { symbol: '●', label: 'High', color: 'text-orange-400', description: 'Thorough analysis, more thinking' },
};

export function EffortCallout({ level, onDone }: Props): ReactNode {
  return (
    <div data-testid="effort-callout" className="rounded-lg border border-gray-200 bg-white p-4 shadow-sm dark:border-gray-700 dark:bg-gray-800">
      <h4 className="text-sm font-semibold text-gray-900 dark:text-gray-100">Effort Level</h4>
      <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">Choose how much effort to use for this task.</p>

      <div className="mt-3 flex items-center gap-3 text-xs text-gray-500 dark:text-gray-400">
        <span className={cn(EFFORT_CONFIG.low.color)}>{EFFORT_CONFIG.low.symbol} low</span>
        <span className={cn(EFFORT_CONFIG.medium.color)}>{EFFORT_CONFIG.medium.symbol} medium</span>
        <span className={cn(EFFORT_CONFIG.high.color)}>{EFFORT_CONFIG.high.symbol} high</span>
      </div>

      <div data-testid="effort-options" className="mt-3 flex flex-col gap-1">
        {(Object.entries(EFFORT_CONFIG) as [EffortLevel, typeof EFFORT_CONFIG.high][]).map(([key, cfg]) => (
          <button
            key={key}
            data-testid={`effort-${key}`}
            onClick={() => onDone?.(key)}
            className={cn(
              'flex items-center gap-2 rounded px-3 py-2 text-left text-sm transition-colors',
              level === key
                ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                : 'hover:bg-gray-100 dark:hover:bg-gray-700',
            )}
          >
            <span className={cn(cfg.color)}>{cfg.symbol}</span>
            <span className="font-medium">{cfg.label}</span>
            <span className="ml-1 text-xs text-gray-500">{cfg.description}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
