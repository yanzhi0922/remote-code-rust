import React from 'react';
import { BarChart3 } from 'lucide-react';

export interface StatsData {
  totalSessions: number;
  totalTokens: number;
  totalCost: number;
  modelsUsed: Record<string, number>;
}

type Props = {
  stats?: StatsData | null;
  loading?: boolean;
  onClose?: () => void;
};

export function Stats({ stats, loading = false, onClose: _onClose }: Props): React.ReactElement {
  if (loading) {
    return (
      <div data-testid="stats-loading" className="mt-2 flex items-center gap-2">
        <div className="h-4 w-4 animate-spin rounded-full border-2 border-cyan-500 border-t-transparent" />
        <span className="text-sm text-gray-500 dark:text-gray-400">Loading stats…</span>
      </div>
    );
  }

  if (!stats) {
    return (
      <div data-testid="stats-empty" className="mt-2">
        <span className="text-sm text-gray-500 dark:text-gray-400">No stats available.</span>
      </div>
    );
  }

  return (
    <div data-testid="stats" className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <BarChart3 className="h-5 w-5 text-cyan-500" />
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Stats</h3>
      </div>

      <div className="grid grid-cols-3 gap-4">
        <div className="flex flex-col rounded-md bg-gray-50 p-3 dark:bg-gray-800">
          <span className="text-xs text-gray-500 dark:text-gray-400">Sessions</span>
          <span className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            {stats.totalSessions.toLocaleString()}
          </span>
        </div>
        <div className="flex flex-col rounded-md bg-gray-50 p-3 dark:bg-gray-800">
          <span className="text-xs text-gray-500 dark:text-gray-400">Tokens</span>
          <span className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            {formatNumber(stats.totalTokens)}
          </span>
        </div>
        <div className="flex flex-col rounded-md bg-gray-50 p-3 dark:bg-gray-800">
          <span className="text-xs text-gray-500 dark:text-gray-400">Cost</span>
          <span className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            ${stats.totalCost.toFixed(2)}
          </span>
        </div>
      </div>

      {Object.keys(stats.modelsUsed).length > 0 && (
        <div className="flex flex-col">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Models Used:</span>
          {Object.entries(stats.modelsUsed).map(([model, count]) => (
            <div key={model} className="flex items-center justify-between px-2 py-1">
              <span className="text-sm text-gray-600 dark:text-gray-400">{model}</span>
              <span className="text-sm text-gray-500 dark:text-gray-400">
                {count.toLocaleString()} sessions
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toLocaleString();
}
