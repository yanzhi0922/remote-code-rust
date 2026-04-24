/**
 * TokenIndicator — Token 使用指示器。
 *
 * 显示 token 数量（格式化为 K/M），带可选进度条。
 * 颜色：<50% 绿色, 50-80% 黄色, >80% 红色。
 */

export interface TokenIndicatorProps {
  usage: {
    inputTokens: number;
    outputTokens: number;
    totalTokens: number;
    maxTokens?: number;
  };
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function getColorClass(ratio: number): string {
  if (ratio > 0.8) return 'bg-red-500';
  if (ratio > 0.5) return 'bg-yellow-500';
  return 'bg-green-500';
}

function getTextColorClass(ratio: number): string {
  if (ratio > 0.8) return 'text-red-600';
  if (ratio > 0.5) return 'text-yellow-600';
  return 'text-green-600';
}

export function TokenIndicator({ usage }: TokenIndicatorProps) {
  const { inputTokens, outputTokens, totalTokens, maxTokens } = usage;
  const ratio = maxTokens ? totalTokens / maxTokens : 0;
  const hasMax = maxTokens !== undefined && maxTokens > 0;

  return (
    <div className="rounded-xl border border-slate-200 bg-white px-3 py-2" data-testid="token-indicator">
      {/* Token counts */}
      <div className="flex items-center gap-4 text-xs">
        <span className="text-slate-500">
          输入: <span className="font-medium text-slate-700" data-testid="input-tokens">{formatTokens(inputTokens)}</span>
        </span>
        <span className="text-slate-500">
          输出: <span className="font-medium text-slate-700" data-testid="output-tokens">{formatTokens(outputTokens)}</span>
        </span>
        <span className={hasMax ? getTextColorClass(ratio) : 'text-slate-700'}>
          总计: <span className="font-medium" data-testid="total-tokens">{formatTokens(totalTokens)}</span>
          {hasMax && <span className="text-slate-400"> / {formatTokens(maxTokens!)}</span>}
        </span>
      </div>

      {/* Progress bar */}
      {hasMax && (
        <div className="mt-2 h-1.5 w-full rounded-full bg-slate-200" data-testid="token-progress">
          <div
            className={`h-1.5 rounded-full transition-all duration-300 ${getColorClass(ratio)}`}
            style={{ width: `${Math.min(ratio * 100, 100)}%` }}
          />
        </div>
      )}
    </div>
  );
}
