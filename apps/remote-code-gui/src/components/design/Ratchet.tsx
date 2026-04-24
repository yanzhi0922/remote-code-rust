import { cn } from '../../lib/utils';

export interface RatchetProps {
  value: number;
  max: number;
  label?: string;
  className?: string;
}

export function Ratchet({ value, max, label, className }: RatchetProps) {
  const percentage = max > 0 ? Math.min((value / max) * 100, 100) : 0;

  return (
    <div data-testid="ratchet" className={cn('space-y-1', className)}>
      {(label || max > 0) && (
        <div className="flex items-center justify-between">
          {label && (
            <span data-testid="ratchet-label" className="text-xs font-medium text-slate-700">
              {label}
            </span>
          )}
          <span data-testid="ratchet-value" className="text-xs text-slate-500">
            {value} / {max}
          </span>
        </div>
      )}
      <div
        data-testid="ratchet-track"
        className="h-2 w-full overflow-hidden rounded-full bg-slate-200"
      >
        <div
          data-testid="ratchet-fill"
          className={cn(
            'h-full rounded-full transition-all duration-300',
            percentage >= 100 ? 'bg-green-500' : percentage >= 75 ? 'bg-yellow-500' : 'bg-blue-500'
          )}
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  );
}
