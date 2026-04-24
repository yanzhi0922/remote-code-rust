import { useState, useMemo } from 'react';
import { Search } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface FuzzyPickerProps {
  items: string[];
  onSelect: (item: string) => void;
  placeholder?: string;
  className?: string;
}

function fuzzyMatch(query: string, text: string): boolean {
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  if (t.includes(q)) return true;

  let qi = 0;
  for (let ti = 0; ti < t.length && qi < q.length; ti++) {
    if (t[ti] === q[qi]) qi++;
  }
  return qi === q.length;
}

export function FuzzyPicker({ items, onSelect, placeholder = '搜索...', className }: FuzzyPickerProps) {
  const [query, setQuery] = useState('');

  const filtered = useMemo(() => {
    if (!query.trim()) return items;
    return items.filter((item) => fuzzyMatch(query, item));
  }, [items, query]);

  return (
    <div data-testid="fuzzy-picker" className={cn('space-y-2', className)}>
      <div className="relative">
        <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400" />
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={placeholder}
          data-testid="fuzzy-picker-input"
          className="w-full rounded-lg border border-slate-200 bg-white py-2 pl-9 pr-3 text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-200"
        />
      </div>

      {filtered.length > 0 ? (
        <ul data-testid="fuzzy-picker-list" className="max-h-48 overflow-y-auto rounded-lg border border-slate-200 bg-white">
          {filtered.map((item) => (
            <li key={item}>
              <button
                type="button"
                data-testid={`fuzzy-picker-item-${item}`}
                onClick={() => {
                  onSelect(item);
                  setQuery('');
                }}
                className="w-full px-3 py-2 text-left text-sm text-slate-700 transition-colors hover:bg-blue-50 hover:text-blue-700"
              >
                {item}
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p data-testid="fuzzy-picker-empty" className="py-2 text-center text-sm text-slate-400">
          无匹配结果
        </p>
      )}
    </div>
  );
}
