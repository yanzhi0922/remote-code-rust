import { Target, Pause, Play, X, BarChart3 } from 'lucide-react';
import type { CodexGoalState, CodexThreadGoalInfo } from '../../lib/types';
import { useAppStore } from '../../stores/useAppStore';

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function formatTime(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  if (m < 60) return `${m}m ${s}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}

function statusBadge(status: CodexThreadGoalInfo['status']): { label: string; color: string } {
  switch (status) {
    case 'Active': return { label: 'Active', color: 'bg-emerald-100 text-emerald-800 border-emerald-300' };
    case 'Paused': return { label: 'Paused', color: 'bg-amber-100 text-amber-800 border-amber-300' };
    case 'BudgetLimited': return { label: 'Budget Exceeded', color: 'bg-orange-100 text-orange-800 border-orange-300' };
    case 'Complete': return { label: 'Complete', color: 'bg-blue-100 text-blue-800 border-blue-300' };
    default: return { label: status, color: 'bg-slate-100 text-slate-800 border-slate-300' };
  }
}

export function GoalStatusBar() {
  const goalState = useAppStore((s) => s.goalState);

  if (!goalState) return null;

  const { goal } = goalState;
  const badge = statusBadge(goal.status);
  const progressPct = goal.tokenBudget
    ? Math.min(100, Math.round((goal.tokensUsed / goal.tokenBudget) * 100))
    : 0;
  const usageLabel = goal.tokenBudget
    ? `${formatTokens(goal.tokensUsed)} / ${formatTokens(goal.tokenBudget)}`
    : `${formatTokens(goal.tokensUsed)} tokens | ${formatTime(goal.timeUsedSeconds)}`;

  return (
    <div
      data-testid="goal-status-bar"
      className="mx-3 mb-2 rounded-lg border border-slate-200 bg-white px-4 py-2.5 shadow-sm"
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <Target className="h-4 w-4 flex-shrink-0 text-emerald-600" />
          <span className="truncate text-sm font-medium text-slate-800">{goal.objective}</span>
          <span className={`flex-shrink-0 rounded-full border px-2 py-0.5 text-xs font-semibold ${badge.color}`}>
            {badge.label}
          </span>
        </div>
      </div>

      {/* Token / time usage bar */}
      <div className="mt-2 flex items-center gap-2">
        <BarChart3 className="h-3.5 w-3.5 flex-shrink-0 text-slate-400" />
        {goal.tokenBudget ? (
          <div className="flex-1 h-2 rounded-full bg-slate-100 overflow-hidden">
            <div
              className={`h-full rounded-full transition-all duration-700 ${
                progressPct > 90 ? 'bg-orange-400' : progressPct > 70 ? 'bg-amber-400' : 'bg-emerald-400'
              }`}
              style={{ width: `${Math.max(2, progressPct)}%` }}
            />
          </div>
        ) : null}
        <span className="flex-shrink-0 text-xs text-slate-500">{usageLabel}</span>
      </div>
    </div>
  );
}
