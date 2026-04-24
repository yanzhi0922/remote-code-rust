import { ChevronDown, ChevronRight, Search } from 'lucide-react';
import { useState } from 'react';
import { cn } from '../../lib/utils';

export interface CollapsedReadSearchContentProps {
  query: string;
  resultCount?: number;
  results?: string[];
  className?: string;
}

export function CollapsedReadSearchContent({
  query,
  resultCount,
  results,
  className,
}: CollapsedReadSearchContentProps) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div
      className={cn(
        'rounded-lg border border-slate-200 bg-slate-50 dark:border-slate-700 dark:bg-slate-800/50',
        className,
      )}
      data-testid="collapsed-search-content"
    >
      <button
        className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-slate-600 hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-slate-700/50"
        onClick={() => setExpanded((v) => !v)}
        data-testid="collapsed-search-toggle"
      >
        {expanded ? (
          <ChevronDown className="h-4 w-4 shrink-0" />
        ) : (
          <ChevronRight className="h-4 w-4 shrink-0" />
        )}
        <Search className="h-4 w-4 shrink-0 text-slate-400" />
        <span className="truncate font-medium">{query}</span>
        {resultCount != null && (
          <span className="ml-auto text-xs text-slate-400">
            {resultCount} 条结果
          </span>
        )}
      </button>
      {expanded && results && results.length > 0 && (
        <div className="border-t border-slate-200 px-3 py-2 dark:border-slate-700">
          <ul className="space-y-1 text-xs text-slate-600 dark:text-slate-400">
            {results.map((r, i) => (
              <li key={i} className="truncate">{r}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
