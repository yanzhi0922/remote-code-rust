import { useState, useMemo, useRef, useCallback, useEffect } from 'react';
import { Search, X, Loader2, FileText, FolderOpen, Clock } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface SearchItem {
  id: string;
  label: string;
  group: string;
  /** Optional preview text (e.g., matching line content) */
  preview?: string;
  /** Optional line number */
  lineNumber?: number;
}

export interface GlobalSearchDialogProps {
  open: boolean;
  onClose: () => void;
  items: SearchItem[];
  onSelect?: (item: SearchItem) => void;
  /** Debounce delay in ms */
  debounceMs?: number;
  /** Max results to show */
  maxResults?: number;
}

const DEFAULT_DEBOUNCE_MS = 300;
const DEFAULT_MAX_RESULTS = 50;
const MAX_RECENT_SEARCHES = 5;

function highlightMatch(text: string, query: string): React.ReactNode {
  if (!query.trim()) return text;
  const parts: React.ReactNode[] = [];
  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  let lastIndex = 0;
  let matchIndex = lowerText.indexOf(lowerQuery);
  let key = 0;

  while (matchIndex !== -1) {
    if (matchIndex > lastIndex) {
      parts.push(<span key={key++}>{text.slice(lastIndex, matchIndex)}</span>);
    }
    parts.push(
      <mark key={key++} className="bg-yellow-200 text-yellow-900 rounded px-0.5">
        {text.slice(matchIndex, matchIndex + query.length)}
      </mark>,
    );
    lastIndex = matchIndex + query.length;
    matchIndex = lowerText.indexOf(lowerQuery, lastIndex);
  }
  if (lastIndex < text.length) {
    parts.push(<span key={key++}>{text.slice(lastIndex)}</span>);
  }
  return parts.length > 0 ? <>{parts}</> : text;
}

function groupByDirectory(items: SearchItem[]): Map<string, SearchItem[]> {
  const groups = new Map<string, SearchItem[]>();
  for (const item of items) {
    const dir = item.group || '其他';
    const existing = groups.get(dir) || [];
    existing.push(item);
    groups.set(dir, existing);
  }
  return groups;
}

export function GlobalSearchDialog({
  open,
  onClose,
  items,
  onSelect,
  debounceMs = DEFAULT_DEBOUNCE_MS,
  maxResults = DEFAULT_MAX_RESULTS,
}: GlobalSearchDialogProps) {
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [isSearching, setIsSearching] = useState(false);
  const [recentSearches, setRecentSearches] = useState<string[]>(() => {
    try {
      const stored = localStorage.getItem('global-search-recent');
      return stored ? JSON.parse(stored) : [];
    } catch {
      return [];
    }
  });
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const resultsListRef = useRef<HTMLDivElement>(null);

  // Debounced query update
  useEffect(() => {
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current);
    }
    // When query is empty, skip debounce and show results immediately
    if (!query.trim()) {
      setDebouncedQuery('');
      setIsSearching(false);
      return;
    }
    setIsSearching(true);
    debounceTimerRef.current = setTimeout(() => {
      setDebouncedQuery(query);
      setIsSearching(false);
    }, debounceMs);
    return () => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }
    };
  }, [query, debounceMs]);

  // Filter and group results
  const { results, groups, truncated } = useMemo(() => {
    const q = debouncedQuery.trim().toLowerCase();
    let filtered: SearchItem[];
    if (!q) {
      filtered = items.slice(0, maxResults);
    } else {
      filtered = items.filter(
        (item) =>
          item.label.toLowerCase().includes(q) ||
          item.group.toLowerCase().includes(q) ||
          (item.preview && item.preview.toLowerCase().includes(q)),
      );
    }
    const truncated = filtered.length > maxResults;
    filtered = filtered.slice(0, maxResults);
    const groups = groupByDirectory(filtered);
    return { results: filtered, groups, truncated };
  }, [items, debouncedQuery, maxResults]);

  // Reset selection when results change
  useEffect(() => {
    setSelectedIndex(0);
  }, [debouncedQuery]);

  // Focus input when dialog opens
  useEffect(() => {
    if (open) {
      setQuery('');
      setDebouncedQuery('');
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  // Add to recent searches
  const addToRecentSearches = useCallback(
    (q: string) => {
      if (!q.trim()) return;
      setRecentSearches((prev) => {
        const next = [q, ...prev.filter((s) => s !== q)].slice(0, MAX_RECENT_SEARCHES);
        try {
          localStorage.setItem('global-search-recent', JSON.stringify(next));
        } catch {
          // ignore storage errors
        }
        return next;
      });
    },
    [],
  );

  // Handle item selection
  const handleSelect = useCallback(
    (item: SearchItem) => {
      addToRecentSearches(query);
      onSelect?.(item);
    },
    [onSelect, query, addToRecentSearches],
  );

  // Keyboard navigation
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((prev) => (prev < results.length - 1 ? prev + 1 : 0));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((prev) => (prev > 0 ? prev - 1 : results.length - 1));
      } else if (e.key === 'Enter') {
        e.preventDefault();
        const item = results[selectedIndex];
        if (item) handleSelect(item);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    },
    [results, selectedIndex, handleSelect, onClose],
  );

  // Scroll selected item into view
  useEffect(() => {
    if (resultsListRef.current) {
      const selected = resultsListRef.current.querySelector('[data-selected="true"]');
      selected?.scrollIntoView({ block: 'nearest' });
    }
  }, [selectedIndex]);

  if (!open) return null;

  const showRecentSearches = !query.trim() && recentSearches.length > 0;

  return (
    <div
      data-testid="global-search-dialog"
      className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh]"
    >
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/40"
        data-testid="global-search-backdrop"
        onClick={onClose}
      />

      {/* Dialog */}
      <div className="relative z-10 w-full max-w-lg rounded-lg border border-slate-200 bg-white shadow-xl">
        {/* Search input */}
        <div className="flex items-center gap-2 border-b border-slate-100 px-4 py-3">
          <Search className="h-4 w-4 shrink-0 text-slate-400" />
          <input
            ref={inputRef}
            data-testid="global-search-input"
            type="text"
            className="flex-1 bg-transparent text-sm outline-none"
            placeholder="搜索文件、命令..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
          />
          {isSearching && (
            <Loader2 className="h-4 w-4 shrink-0 animate-spin text-blue-400" data-testid="global-search-loading" />
          )}
          <button
            type="button"
            className="rounded p-1 hover:bg-slate-100"
            onClick={onClose}
            title="关闭"
          >
            <X className="h-4 w-4 text-slate-400" />
          </button>
        </div>

        {/* Results */}
        <div
          ref={resultsListRef}
          className="max-h-[50vh] overflow-y-auto p-2"
          data-testid="global-search-results"
        >
          {/* Loading state */}
          {isSearching && query.trim() && (
            <div className="flex items-center justify-center gap-2 py-4 text-sm text-slate-400">
              <Loader2 className="h-4 w-4 animate-spin" />
              搜索中...
            </div>
          )}

          {/* Empty state */}
          {!isSearching && debouncedQuery.trim() && results.length === 0 && (
            <div
              data-testid="global-search-empty"
              className="py-6 text-center"
            >
              <Search className="mx-auto mb-2 h-8 w-8 text-slate-300" />
              <p className="text-sm text-slate-400">未找到匹配结果</p>
              <p className="mt-1 text-xs text-slate-300">
                尝试使用不同的关键词搜索
              </p>
            </div>
          )}

          {/* Recent searches */}
          {showRecentSearches && (
            <div data-testid="global-search-recent">
              <div className="flex items-center gap-1.5 px-2 py-1.5 text-xs font-medium text-slate-400">
                <Clock className="h-3 w-3" />
                最近搜索
              </div>
              {recentSearches.map((term) => (
                <button
                  key={term}
                  type="button"
                  className="flex w-full items-center gap-2 rounded px-3 py-1.5 text-left text-sm hover:bg-slate-50"
                  onClick={() => setQuery(term)}
                  data-testid={`global-search-recent-${term}`}
                >
                  <Clock className="h-3 w-3 text-slate-300" />
                  <span className="text-slate-600">{term}</span>
                </button>
              ))}
            </div>
          )}

          {/* Grouped results */}
          {!isSearching && results.length > 0 && !showRecentSearches && (
            <>
              {Array.from(groups.entries()).map(([group, groupItems]) => (
                <div key={group} className="mb-2">
                  {/* Group header */}
                  <div className="flex items-center gap-1.5 px-2 py-1.5 text-xs font-medium text-slate-400">
                    <FolderOpen className="h-3 w-3" />
                    {group}
                    <span className="text-slate-300">({groupItems.length})</span>
                  </div>
                  {/* Group items */}
                  {groupItems.map((item) => {
                    const globalIndex = results.indexOf(item);
                    const isSelected = globalIndex === selectedIndex;
                    return (
                      <button
                        key={item.id}
                        type="button"
                        data-testid={`global-search-item-${item.id}`}
                        data-selected={isSelected}
                        className={cn(
                          'flex w-full items-start gap-2 rounded px-3 py-2 text-left text-sm transition-colors',
                          isSelected ? 'bg-blue-50 ring-1 ring-blue-200' : 'hover:bg-slate-50',
                        )}
                        onClick={() => handleSelect(item)}
                        onMouseEnter={() => setSelectedIndex(globalIndex)}
                      >
                        <FileText className="mt-0.5 h-4 w-4 shrink-0 text-slate-400" />
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            <span className="text-slate-700 truncate">
                              {highlightMatch(item.label, debouncedQuery)}
                            </span>
                            {item.lineNumber !== undefined && (
                              <span className="shrink-0 text-[10px] text-slate-300">
                                :{item.lineNumber}
                              </span>
                            )}
                          </div>
                          {item.preview && (
                            <p className="mt-0.5 truncate text-xs text-slate-400">
                              {highlightMatch(item.preview, debouncedQuery)}
                            </p>
                          )}
                        </div>
                      </button>
                    );
                  })}
                </div>
              ))}
              {/* Truncation notice */}
              {truncated && (
                <div className="px-3 py-2 text-center text-xs text-slate-400">
                  显示前 {maxResults} 个结果，请缩小搜索范围
                </div>
              )}
            </>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between border-t border-slate-100 px-4 py-2 text-[10px] text-slate-300">
          <div className="flex items-center gap-3">
            <span>↑↓ 导航</span>
            <span>Enter 选择</span>
            <span>Esc 关闭</span>
          </div>
          {results.length > 0 && (
            <span>
              {results.length}
              {truncated ? '+' : ''} 个结果
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
