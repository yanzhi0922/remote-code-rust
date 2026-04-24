import { type ReactNode, useState, useMemo } from 'react';
import { Search, FileText, X } from 'lucide-react';
import { cn } from '../../lib/utils';

interface QuickOpenResult {
  path: string;
  label: string;
}

interface Props {
  onDone: () => void;
  onInsert?: (text: string) => void;
  results?: QuickOpenResult[];
}

export function QuickOpenDialog({ onDone, onInsert, results: externalResults }: Props): ReactNode {
  const [query, setQuery] = useState('');
  const [focusedIndex, setFocusedIndex] = useState(0);

  const defaultResults: QuickOpenResult[] = useMemo(() => [], []);
  const results = externalResults ?? defaultResults;

  const filtered = useMemo(() => {
    if (!query.trim()) return results;
    const q = query.toLowerCase();
    return results.filter(
      (r) => r.path.toLowerCase().includes(q) || r.label.toLowerCase().includes(q),
    );
  }, [query, results]);

  return (
    <div
      data-testid="quick-open-dialog"
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[15vh]"
    >
      <div className="mx-4 w-full max-w-lg rounded-lg border border-gray-200 bg-white shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center gap-2 border-b border-gray-200 px-3 py-2 dark:border-gray-700">
          <Search className="h-4 w-4 text-gray-400" />
          <input
            data-testid="quick-open-input"
            type="text"
            className="flex-1 bg-transparent text-sm text-gray-900 outline-none placeholder:text-gray-400 dark:text-gray-100"
            value={query}
            onChange={(e) => { setQuery(e.target.value); setFocusedIndex(0); }}
            placeholder="Search files..."
            autoFocus
          />
          <button
            data-testid="quick-open-close"
            onClick={onDone}
            aria-label="Close"
            className="text-gray-400 hover:text-gray-600"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {filtered.length > 0 ? (
          <div data-testid="quick-open-results" className="max-h-64 overflow-y-auto">
            {filtered.map((result, i) => (
              <button
                key={result.path}
                data-testid={`quick-open-result-${i}`}
                onClick={() => {
                  onInsert?.(result.path);
                  onDone();
                }}
                onMouseEnter={() => setFocusedIndex(i)}
                className={cn(
                  'flex w-full items-center gap-2 px-3 py-2 text-left text-sm',
                  i === focusedIndex
                    ? 'bg-blue-50 dark:bg-blue-900/30'
                    : 'hover:bg-gray-50 dark:hover:bg-gray-700/50',
                )}
              >
                <FileText className="h-4 w-4 shrink-0 text-gray-400" />
                <span className="truncate text-gray-800 dark:text-gray-200">{result.label}</span>
                <span className="ml-auto shrink-0 text-xs text-gray-400">{result.path}</span>
              </button>
            ))}
          </div>
        ) : (
          query.trim() && (
            <div data-testid="quick-open-empty" className="px-3 py-4 text-center text-sm text-gray-500">
              No results found
            </div>
          )
        )}
      </div>
    </div>
  );
}
