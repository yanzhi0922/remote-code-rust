import { BarChart3, DollarSign, Clock, Zap } from 'lucide-react';

export interface UsageStats {
  totalTokens: number;
  inputTokens: number;
  outputTokens: number;
  totalCost: number;
  sessionCount: number;
  averageResponseTime: number;
}

export interface UsageProps {
  stats: UsageStats;
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

export function Usage({ stats }: UsageProps) {
  return (
    <div data-testid="usage-panel" className="space-y-4 p-4">
      <h2 className="text-sm font-semibold text-slate-800">使用量统计</h2>
      <div className="grid grid-cols-2 gap-3">
        <div data-testid="usage-total-tokens" className="rounded-lg border border-slate-200 p-3">
          <div className="flex items-center gap-1.5 text-slate-500">
            <BarChart3 className="h-4 w-4" />
            <span className="text-xs">总Token</span>
          </div>
          <p className="mt-1 text-lg font-semibold text-slate-800">{formatNumber(stats.totalTokens)}</p>
        </div>
        <div data-testid="usage-cost" className="rounded-lg border border-slate-200 p-3">
          <div className="flex items-center gap-1.5 text-slate-500">
            <DollarSign className="h-4 w-4" />
            <span className="text-xs">总费用</span>
          </div>
          <p className="mt-1 text-lg font-semibold text-slate-800">${stats.totalCost.toFixed(2)}</p>
        </div>
        <div data-testid="usage-sessions" className="rounded-lg border border-slate-200 p-3">
          <div className="flex items-center gap-1.5 text-slate-500">
            <Clock className="h-4 w-4" />
            <span className="text-xs">会话数</span>
          </div>
          <p className="mt-1 text-lg font-semibold text-slate-800">{stats.sessionCount}</p>
        </div>
        <div data-testid="usage-response-time" className="rounded-lg border border-slate-200 p-3">
          <div className="flex items-center gap-1.5 text-slate-500">
            <Zap className="h-4 w-4" />
            <span className="text-xs">平均响应</span>
          </div>
          <p className="mt-1 text-lg font-semibold text-slate-800">{stats.averageResponseTime}ms</p>
        </div>
      </div>
      <div className="rounded-lg border border-slate-200 p-3">
        <h3 className="mb-2 text-xs font-medium text-slate-500">Token 使用明细</h3>
        <div className="flex gap-4 text-sm">
          <span className="text-slate-600">输入: <strong>{formatNumber(stats.inputTokens)}</strong></span>
          <span className="text-slate-600">输出: <strong>{formatNumber(stats.outputTokens)}</strong></span>
        </div>
      </div>
    </div>
  );
}
