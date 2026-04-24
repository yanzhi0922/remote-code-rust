import { type ReactNode } from 'react';
import { BarChart3 } from 'lucide-react';
import { cn } from '../../lib/utils';

interface ContextCategory {
  name: string;
  tokens: number;
  color: string;
}

interface Props {
  categories: ContextCategory[];
  totalTokens: number;
  maxTokens: number;
  percentage: number;
  model: string;
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export function ContextVisualization({
  categories,
  totalTokens,
  maxTokens,
  percentage,
  model,
}: Props): ReactNode {
  const barWidth = Math.min(100, percentage);

  return (
    <div data-testid="context-visualization" className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <BarChart3 className="h-4 w-4 text-gray-500" />
        <h4 className="text-sm font-semibold text-gray-900 dark:text-gray-100">Context Usage</h4>
      </div>

      {/* Token bar */}
      <div data-testid="context-bar" className="h-4 w-full rounded-full bg-gray-200 dark:bg-gray-700">
        <div
          data-testid="context-bar-fill"
          className={cn(
            'h-4 rounded-full transition-all',
            percentage > 90 ? 'bg-red-500' : percentage > 70 ? 'bg-yellow-500' : 'bg-green-500',
          )}
          style={{ width: `${barWidth}%` }}
        />
      </div>

      {/* Token count */}
      <p className="text-xs text-gray-500 dark:text-gray-400">
        {model} · {formatTokens(totalTokens)}/{formatTokens(maxTokens)} tokens ({percentage}%)
      </p>

      {/* Category breakdown */}
      {categories.length > 0 && (
        <div data-testid="context-categories" className="flex flex-col gap-1">
          {categories.map((cat) => {
            const catPercent = maxTokens > 0 ? (cat.tokens / maxTokens) * 100 : 0;
            return (
              <div key={cat.name} className="flex items-center gap-2 text-xs">
                <div
                  className={cn('h-3 rounded', cat.color)}
                  style={{ width: `${Math.max(2, catPercent)}%`, minWidth: '4px' }}
                />
                <span className="text-gray-600 dark:text-gray-400">{cat.name}</span>
                <span className="ml-auto text-gray-500">{formatTokens(cat.tokens)}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
