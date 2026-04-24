import { useState, useMemo } from 'react';
import { Search, X } from 'lucide-react';

export interface GlobalSearchDialogProps {
  open: boolean;
  onClose: () => void;
  items: Array<{ id: string; label: string; group: string }>;
  onSelect?: (item: { id: string; label: string; group: string }) => void;
}

export function GlobalSearchDialog({ open, onClose, items, onSelect }: GlobalSearchDialogProps) {
  const [query, setQuery] = useState('');

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items.slice(0, 50);
    return items.filter(
      (item) => item.label.toLowerCase().includes(q) || item.group.toLowerCase().includes(q),
    );
  }, [items, query]);

  if (!open) return null;

  return (
    <div data-testid="global-search-dialog" className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh]">
      <div className="fixed inset-0 bg-black/40" data-testid="global-search-backdrop" onClick={onClose} />
      <div className="relative z-10 w-full max-w-lg rounded-lg border border-slate-200 bg-white shadow-xl">
        <div className="flex items-center gap-2 border-b border-slate-100 px-4 py-3">
          <Search className="h-4 w-4 text-slate-400" />
          <input
            data-testid="global-search-input"
            type="text"
            className="flex-1 bg-transparent text-sm outline-none"
            placeholder="搜索..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoFocus
          />
          <button type="button" className="rounded p-1 hover:bg-slate-100" onClick={onClose} title="关闭">
            <X className="h-4 w-4 text-slate-400" />
          </button>
        </div>
        <div className="max-h-[50vh] overflow-y-auto p-2">
          {filtered.length === 0 ? (
            <div data-testid="global-search-empty" className="py-4 text-center text-sm text-slate-400">
              无结果
            </div>
          ) : (
            filtered.map((item) => (
              <button
                key={item.id}
                type="button"
                data-testid={`global-search-item-${item.id}`}
                className="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm hover:bg-slate-50"
                onClick={() => onSelect?.(item)}
              >
                <span className="text-slate-700">{item.label}</span>
                <span className="text-xs text-slate-400">{item.group}</span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
