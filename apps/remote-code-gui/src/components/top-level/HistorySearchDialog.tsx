import { useState, useMemo } from 'react';
import { History, Search, X } from 'lucide-react';

export interface HistoryEntry {
  id: string;
  text: string;
  timestamp: string;
}

export interface HistorySearchDialogProps {
  open: boolean;
  onClose: () => void;
  entries: HistoryEntry[];
  onSelect?: (entry: HistoryEntry) => void;
}

export function HistorySearchDialog({ open, onClose, entries, onSelect }: HistorySearchDialogProps) {
  const [query, setQuery] = useState('');

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter((entry) => entry.text.toLowerCase().includes(q));
  }, [entries, query]);

  if (!open) return null;

  return (
    <div data-testid="history-search-dialog" className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh]">
      <div className="fixed inset-0 bg-black/40" data-testid="history-search-backdrop" onClick={onClose} />
      <div className="relative z-10 w-full max-w-lg rounded-lg border border-slate-200 bg-white shadow-xl">
        <div className="flex items-center gap-2 border-b border-slate-100 px-4 py-3">
          <History className="h-4 w-4 text-slate-400" />
          <Search className="h-4 w-4 text-slate-400" />
          <input
            data-testid="history-search-input"
            type="text"
            className="flex-1 bg-transparent text-sm outline-none"
            placeholder="搜索历史..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoFocus
          />
          <button type="button" className="rounded p-1 hover:bg-slate-100" onClick={onClose} title="关闭">
            <X className="h-4 w-4 text-slate-400" />
          </button>
        </div>
        <div className="max-h-[50vh] overflow-y-auto">
          {filtered.length === 0 ? (
            <div data-testid="history-search-empty" className="py-8 text-center text-sm text-slate-400">
              无历史记录
            </div>
          ) : (
            filtered.map((entry) => (
              <button
                key={entry.id}
                type="button"
                data-testid={`history-search-entry-${entry.id}`}
                className="flex w-full items-start gap-2 border-b border-slate-50 px-4 py-2 text-left hover:bg-slate-50"
                onClick={() => onSelect?.(entry)}
              >
                <span className="flex-1 text-sm text-slate-700">{entry.text}</span>
                <span className="shrink-0 text-xs text-slate-400">{entry.timestamp}</span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
