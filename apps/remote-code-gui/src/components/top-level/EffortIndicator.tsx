import { cn } from '../../lib/utils';

export type EffortLevel = 'low' | 'medium' | 'high' | 'max';

export interface EffortIndicatorProps {
  level: EffortLevel;
  showLabel?: boolean;
}

const EFFORT_SYMBOLS: Record<EffortLevel, string> = {
  low: '○',
  medium: '◐',
  high: '●',
  max: '◉',
};

const EFFORT_COLORS: Record<EffortLevel, string> = {
  low: 'text-slate-400',
  medium: 'text-blue-500',
  high: 'text-blue-700',
  max: 'text-purple-600',
};

export function EffortIndicator({ level, showLabel = true }: EffortIndicatorProps) {
  return (
    <span data-testid="effort-indicator" className="inline-flex items-center gap-1">
      <span className={cn('text-sm', EFFORT_COLORS[level])}>{EFFORT_SYMBOLS[level]}</span>
      {showLabel && (
        <span className="text-xs text-slate-500">{level}</span>
      )}
    </span>
  );
}
