/**
 * ProgressBar — 水平进度条组件。
 *
 * 支持自定义颜色、百分比标签和两种尺寸。
 */

export interface ProgressBarProps {
  value: number;
  max: number;
  color?: string;
  showLabel?: boolean;
  size?: 'sm' | 'md';
}

const sizeClassMap: Record<NonNullable<ProgressBarProps['size']>, string> = {
  sm: 'h-1.5',
  md: 'h-3',
};

export function ProgressBar({
  value,
  max,
  color = 'bg-blue-500',
  showLabel = false,
  size = 'md',
}: ProgressBarProps) {
  const pct = max > 0 ? Math.min((value / max) * 100, 100) : 0;

  return (
    <div className="w-full" data-testid="progress-bar">
      <div
        className={`w-full rounded-full bg-slate-200 ${sizeClassMap[size]}`}
      >
        <div
          className={`${sizeClassMap[size]} rounded-full transition-all duration-300 ${color}`}
          style={{ width: `${pct}%` }}
          role="progressbar"
          aria-valuenow={value}
          aria-valuemin={0}
          aria-valuemax={max}
        />
      </div>
      {showLabel && (
        <span className="mt-1 text-xs text-slate-500" data-testid="progress-label">
          {Math.round(pct)}%
        </span>
      )}
    </div>
  );
}
