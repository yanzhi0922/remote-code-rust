import { Clock, Trash2 } from 'lucide-react';

export interface HistorySearchProps {
  history: Array<{ id: string; query: string; timestamp: string }>;
  onSelect: (id: string) => void;
  onClearHistory: () => void;
}

export function HistorySearch({ history, onSelect, onClearHistory }: HistorySearchProps) {
  if (history.length === 0) {
    return (
      <div data-testid="history-empty" className="px-4 py-8 text-center text-sm text-slate-400">
        暂无搜索历史
      </div>
    );
  }

  return (
    <div data-testid="history-search">
      <div className="flex items-center justify-between border-b border-slate-100 px-3 py-2">
        <span className="text-xs font-medium text-slate-500">搜索历史</span>
        <button
          type="button"
          onClick={onClearHistory}
          data-testid="clear-history"
          className="flex items-center gap-1 text-xs text-slate-400 hover:text-red-500"
        >
          <Trash2 className="h-3 w-3" />
          清除历史
        </button>
      </div>
      <ul className="max-h-60 overflow-y-auto">
        {history.map((item) => (
          <li key={item.id}>
            <button
              type="button"
              data-testid={`history-item-${item.id}`}
              className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-slate-700 hover:bg-slate-50"
              onClick={() => onSelect(item.id)}
            >
              <Clock className="h-3.5 w-3.5 shrink-0 text-slate-400" />
              <span className="min-w-0 flex-1 truncate">{item.query}</span>
              <span className="shrink-0 text-xs text-slate-400">{item.timestamp}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
