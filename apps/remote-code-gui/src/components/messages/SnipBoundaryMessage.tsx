import { Scissors } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface SnipBoundaryMessageProps {
  entriesRemoved?: number;
  summary?: string;
  className?: string;
}

export function SnipBoundaryMessage({
  entriesRemoved,
  summary,
  className,
}: SnipBoundaryMessageProps) {
  return (
    <div
      className={cn(
        'flex items-center gap-2 rounded-lg border border-dashed border-slate-300 bg-slate-50 px-4 py-2 text-xs text-slate-500 dark:border-slate-600 dark:bg-slate-800/30 dark:text-slate-400',
        className,
      )}
      data-testid="snip-boundary-message"
    >
      <Scissors className="h-3.5 w-3.5 shrink-0" />
      <span>
        内容已剪切
        {entriesRemoved != null ? ` (${entriesRemoved} 条)` : ''}
      </span>
      {summary && (
        <span className="ml-1 truncate text-slate-400">— {summary}</span>
      )}
    </div>
  );
}
