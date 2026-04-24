import { useState, useCallback, useEffect } from 'react';
import { X } from 'lucide-react';
import { SearchInput } from './SearchInput';
import { SearchResults, type SearchResult } from './SearchResults';
import { HistorySearch } from './HistorySearch';

export type { SearchResult };

export interface GlobalSearchProps {
  visible: boolean;
  onClose: () => void;
  onSelectResult: (result: SearchResult) => void;
}

const SAMPLE_RESULTS: SearchResult[] = [
  { type: 'file', title: 'src/app.tsx', subtitle: '主应用组件' },
  { type: 'file', title: 'src/main.ts', subtitle: '入口文件' },
  { type: 'message', title: '关于性能优化的讨论', subtitle: '2 小时前' },
  { type: 'command', title: '格式化文档', subtitle: 'Shift+Alt+F' },
  { type: 'command', title: '跳转到定义', subtitle: 'F12' },
  { type: 'setting', title: '编辑器字体大小', subtitle: '外观设置' },
  { type: 'setting', title: '主题', subtitle: '外观设置' },
];

const RECENT_HISTORY = [
  { id: 'h1', query: 'React hooks', timestamp: '10:30' },
  { id: 'h2', query: 'TypeScript 泛型', timestamp: '09:15' },
];

export function GlobalSearch({ visible, onClose, onSelectResult }: GlobalSearchProps) {
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(-1);

  const filteredResults = query.trim()
    ? SAMPLE_RESULTS.filter(
        (r) =>
          r.title.toLowerCase().includes(query.toLowerCase()) ||
          (r.subtitle && r.subtitle.toLowerCase().includes(query.toLowerCase())),
      )
    : [];

  const resetAndClose = useCallback(() => {
    setQuery('');
    setSelectedIndex(-1);
    onClose();
  }, [onClose]);

  useEffect(() => {
    if (!visible) {
      setQuery('');
      setSelectedIndex(-1);
    }
  }, [visible]);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && visible) {
        resetAndClose();
      }
    };
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [visible, resetAndClose]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    const total = filteredResults.length;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex((prev) => (prev + 1) % total);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex((prev) => (prev - 1 + total) % total);
    } else if (e.key === 'Enter' && selectedIndex >= 0 && selectedIndex < total) {
      onSelectResult(filteredResults[selectedIndex]);
      resetAndClose();
    }
  };

  if (!visible) return null;

  return (
    <div
      data-testid="global-search-overlay"
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/30 pt-[15vh]"
      onClick={(e) => {
        if (e.target === e.currentTarget) resetAndClose();
      }}
    >
      <div
        data-testid="global-search"
        className="w-full max-w-lg rounded-2xl bg-white shadow-2xl"
      >
        <div className="flex items-center gap-2 border-b border-slate-100 px-3 py-2">
          <div className="flex-1">
            <SearchInput
              value={query}
              onChange={(v) => {
                setQuery(v);
                setSelectedIndex(-1);
              }}
              placeholder="搜索消息、文件、命令、设置..."
              onKeyDown={handleKeyDown}
            />
          </div>
          <button
            type="button"
            onClick={resetAndClose}
            title="关闭"
            aria-label="关闭搜索"
            data-testid="global-search-close"
            className="shrink-0 rounded-lg p-1.5 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="max-h-96 overflow-y-auto">
          {query.trim() ? (
            <SearchResults
              results={filteredResults}
              selectedIndex={selectedIndex}
              onSelect={(index) => {
                onSelectResult(filteredResults[index]);
                resetAndClose();
              }}
              onHover={setSelectedIndex}
            />
          ) : (
            <HistorySearch
              history={RECENT_HISTORY}
              onSelect={(id) => {
                const item = RECENT_HISTORY.find((h) => h.id === id);
                if (item) {
                  setQuery(item.query);
                  setSelectedIndex(-1);
                }
              }}
              onClearHistory={() => {}}
            />
          )}
        </div>

        <div className="flex items-center justify-between border-t border-slate-100 px-3 py-1.5 text-xs text-slate-400">
          <span>↑↓ 导航 · Enter 选择 · Esc 关闭</span>
          {query.trim() && (
            <span>{filteredResults.length} 个结果</span>
          )}
        </div>
      </div>
    </div>
  );
}
