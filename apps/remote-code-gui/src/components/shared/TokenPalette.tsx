import { useMemo } from 'react';

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

interface TokenPaletteProps {
  inputTokens: number;
  outputTokens: number;
  maxTokens: number;
  estimatedTokens: number;
}

export function TokenPalette({
  inputTokens,
  outputTokens,
  maxTokens,
  estimatedTokens,
}: TokenPaletteProps) {
  const segments = useMemo(() => {
    const total = inputTokens + outputTokens;
    const remaining = Math.max(0, maxTokens - estimatedTokens);
    const totalForBars = estimatedTokens > 0 ? maxTokens : total || 1;

    const inputPct = totalForBars > 0 ? (inputTokens / totalForBars) * 100 : 0;
    const outputPct = totalForBars > 0 ? (outputTokens / totalForBars) * 100 : 0;
    const remainingPct = totalForBars > 0 ? (remaining / totalForBars) * 100 : 100;
    const usagePct = maxTokens > 0 ? (estimatedTokens / maxTokens) * 100 : 0;

    return {
      inputPct: Math.min(inputPct, 100),
      outputPct: Math.min(outputPct, 100 - inputPct),
      remainingPct: Math.max(0, 100 - usagePct),
      usagePct,
      remaining,
      total,
    };
  }, [inputTokens, outputTokens, maxTokens, estimatedTokens]);

  const usageColor =
    segments.usagePct > 90
      ? 'text-rc-accent-error'
      : segments.usagePct > 75
        ? 'text-rc-accent-warning'
        : 'text-rc-accent-success';

  return (
    <div className="space-y-3">
      {/* Stacked horizontal bar */}
      <div>
        <div className="mb-1 flex items-center justify-between">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-rc-text-tertiary">
            Token Allocation
          </span>
          <span className={`font-mono text-xs ${usageColor}`}>
            {segments.usagePct.toFixed(0)}% used
          </span>
        </div>
        <div className="flex h-3 w-full overflow-hidden rounded-full bg-rc-bg-tertiary">
          <div
            className="h-full bg-rc-accent-info transition-all duration-500"
            style={{ width: `${segments.inputPct}%` }}
            title={`Input: ${formatTokens(inputTokens)}`}
          />
          <div
            className="h-full bg-rc-accent-success transition-all duration-500"
            style={{ width: `${segments.outputPct}%` }}
            title={`Output: ${formatTokens(outputTokens)}`}
          />
          <div
            className="h-full bg-rc-bg-tertiary"
            style={{ width: `${segments.remainingPct}%` }}
          />
        </div>
      </div>

      {/* Breakdown grid */}
      <div className="grid grid-cols-3 gap-2 text-center">
        <div className="rounded-md border border-rc-border-primary bg-rc-bg-elevated px-2 py-1.5">
          <div className="flex items-center justify-center gap-1">
            <span className="inline-block h-2 w-2 rounded-full bg-rc-accent-info" />
            <span className="text-[10px] text-rc-text-tertiary">Input</span>
          </div>
          <div className="mt-0.5 font-mono text-sm text-rc-text-primary">
            {formatTokens(inputTokens)}
          </div>
        </div>
        <div className="rounded-md border border-rc-border-primary bg-rc-bg-elevated px-2 py-1.5">
          <div className="flex items-center justify-center gap-1">
            <span className="inline-block h-2 w-2 rounded-full bg-rc-accent-success" />
            <span className="text-[10px] text-rc-text-tertiary">Output</span>
          </div>
          <div className="mt-0.5 font-mono text-sm text-rc-text-primary">
            {formatTokens(outputTokens)}
          </div>
        </div>
        <div className="rounded-md border border-rc-border-primary bg-rc-bg-elevated px-2 py-1.5">
          <div className="flex items-center justify-center gap-1">
            <span className="inline-block h-2 w-2 rounded-full bg-rc-bg-tertiary" />
            <span className="text-[10px] text-rc-text-tertiary">Free</span>
          </div>
          <div className="mt-0.5 font-mono text-sm text-rc-text-primary">
            {formatTokens(segments.remaining)}
          </div>
        </div>
      </div>

      {/* Summary row */}
      <div className="flex items-center justify-between border-t border-rc-border-secondary pt-2 text-xs text-rc-text-tertiary">
        <span>
          Total: <span className="font-mono text-rc-text-primary">{formatTokens(segments.total)}</span>
        </span>
        <span>
          Window: <span className="font-mono text-rc-text-primary">{formatTokens(maxTokens)}</span>
        </span>
      </div>
    </div>
  );
}
