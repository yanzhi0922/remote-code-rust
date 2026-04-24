import { ChevronRight } from 'lucide-react';
import { cn } from '../../lib/utils';

/** PromptInputFooterSuggestions 组件属性 */
export interface PromptInputFooterSuggestionsProps {
  /** 建议列表 */
  suggestions: string[];
  /** 当前选中索引 */
  selectedIndex: number;
  /** 选中回调 */
  onSelect: (index: number) => void;
  /** 是否可见 */
  visible: boolean;
  /** 额外 CSS 类名 */
  className?: string;
}

/**
 * 自动完成建议列表。
 * 显示在输入框下方，支持键盘导航高亮。
 */
export function PromptInputFooterSuggestions({
  suggestions,
  selectedIndex,
  onSelect,
  visible,
  className,
}: PromptInputFooterSuggestionsProps) {
  if (!visible || suggestions.length === 0) return null;

  return (
    <div
      className={cn(
        'rounded-md border border-slate-200 bg-white shadow-lg dark:border-slate-700 dark:bg-slate-800',
        className,
      )}
      data-testid="prompt-suggestions"
    >
      <ul className="py-1">
        {suggestions.map((suggestion, index) => (
          <li key={index}>
            <button
              type="button"
              className={cn(
                'flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm',
                index === selectedIndex
                  ? 'bg-blue-50 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300'
                  : 'text-slate-700 hover:bg-slate-50 dark:text-slate-300 dark:hover:bg-slate-700',
              )}
              onClick={() => onSelect(index)}
            >
              <ChevronRight className="h-3 w-3 shrink-0" />
              <span className="truncate">{suggestion}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
