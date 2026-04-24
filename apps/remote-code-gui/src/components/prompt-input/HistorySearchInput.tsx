import { cn } from '../../lib/utils';

/** HistorySearchInput 组件属性 */
export interface HistorySearchInputProps {
  /** 是否可见 */
  visible: boolean;
  /** 搜索查询 */
  query: string;
  /** 查询变化回调 */
  onQueryChange: (query: string) => void;
  /** 选中结果回调 */
  onSelect: (entry: string) => void;
  /** 搜索结果列表 */
  results: string[];
  /** 当前选中索引 */
  selectedIndex: number;
  /** 关闭回调 */
  onClose: () => void;
  /** 额外 CSS 类名 */
  className?: string;
}

/**
 * 历史搜索弹出框。
 * 显示搜索输入框和结果列表，支持键盘导航。
 */
export function HistorySearchInput({
  visible,
  query,
  onQueryChange,
  onSelect,
  results,
  selectedIndex,
  onClose,
  className,
}: HistorySearchInputProps) {
  if (!visible) return null;

  return (
    <div
      className={cn(
        'rounded-md border border-slate-200 bg-white shadow-lg dark:border-slate-700 dark:bg-slate-800',
        className,
      )}
      data-testid="history-search-input"
    >
      <div className="border-b border-slate-200 px-3 py-2 dark:border-slate-700">
        <input
          type="text"
          value={query}
          onChange={(e) => onQueryChange(e.target.value)}
          placeholder="搜索历史命令..."
          className="w-full border-0 bg-transparent text-sm text-slate-900 placeholder:text-slate-400 focus:outline-none dark:text-slate-100"
          autoFocus
        />
      </div>
      {results.length > 0 && (
        <ul className="max-h-48 overflow-y-auto py-1">
          {results.map((result, index) => (
            <li key={index}>
              <button
                type="button"
                className={cn(
                  'block w-full px-3 py-1.5 text-left text-sm',
                  index === selectedIndex
                    ? 'bg-blue-50 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300'
                    : 'text-slate-700 hover:bg-slate-50 dark:text-slate-300 dark:hover:bg-slate-700',
                )}
                onClick={() => onSelect(result)}
              >
                <span className="truncate">{result}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="border-t border-slate-200 px-3 py-1.5 text-xs text-slate-400 dark:border-slate-700">
        <button
          type="button"
          onClick={onClose}
          className="hover:text-slate-600 dark:hover:text-slate-300"
        >
          按 Esc 关闭
        </button>
      </div>
    </div>
  );
}
