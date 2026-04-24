import { memo } from 'react';
import { Wrench } from 'lucide-react';
import type { ToolCallInfo } from '../../lib/types';
import { cn, truncateMiddle } from '../../lib/utils';

/** 分组工具使用内容组件属性 */
export interface GroupedToolUseContentProps {
  /** 工具调用列表 */
  toolCalls: ToolCallInfo[];
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 分组工具使用内容渲染组件。
 * 将多个工具调用分组显示，显示工具数量摘要。
 */
export const GroupedToolUseContent = memo(function GroupedToolUseContent({
  toolCalls,
  className,
}: GroupedToolUseContentProps) {
  if (toolCalls.length === 0) {
    return null;
  }

  return (
    <div
      data-testid="grouped-tool-use"
      className={cn(
        'rounded-lg border border-slate-200 bg-slate-50 px-4 py-3 dark:border-slate-700 dark:bg-slate-800/50',
        className,
      )}
    >
      <div className="mb-2 flex items-center gap-2">
        <Wrench className="h-3.5 w-3.5 text-slate-500 dark:text-slate-400" />
        <span className="text-xs font-medium text-slate-600 dark:text-slate-400">
          {toolCalls.length} tool{toolCalls.length > 1 ? 's' : ''} called
        </span>
      </div>
      <div className="space-y-1">
        {toolCalls.map((tc) => {
          const inputStr = typeof tc.input === 'string'
            ? tc.input
            : JSON.stringify(tc.input);
          return (
            <div key={tc.id} className="flex items-center gap-2 text-xs">
              <span className="font-medium text-slate-700 dark:text-slate-300">
                {tc.name}
              </span>
              <span className="text-slate-400 dark:text-slate-500">
                {truncateMiddle(inputStr, 50)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
});
