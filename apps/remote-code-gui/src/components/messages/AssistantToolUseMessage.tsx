import { memo, useState, useCallback } from 'react';
import { Wrench, Loader2, CheckCircle2, XCircle, ChevronRight } from 'lucide-react';
import type { ToolCallInfo, ToolProgressInfo } from '../../lib/types';
import { cn, truncateMiddle } from '../../lib/utils';

/** 助手工具使用消息组件属性 */
export interface AssistantToolUseMessageProps {
  /** 工具调用信息 */
  toolCall: ToolCallInfo;
  /** 是否正在运行 */
  isRunning: boolean;
  /** 是否已完成 */
  isResolved: boolean;
  /** 是否出错 */
  isError: boolean;
  /** 工具进度信息 */
  progress?: ToolProgressInfo;
  /** 是否显示详细信息 */
  verbose?: boolean;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 助手工具使用消息渲染组件。
 * 运行中显示 spinner，已完成显示 ✓，错误显示 ✗。
 */
export const AssistantToolUseMessage = memo(function AssistantToolUseMessage({
  toolCall,
  isRunning,
  isResolved,
  isError,
  progress,
  verbose = false,
  className,
}: AssistantToolUseMessageProps) {
  const [expanded, setExpanded] = useState(false);
  const toggleExpanded = useCallback(() => setExpanded((prev) => !prev), []);

  const inputSummary = typeof toolCall.input === 'string'
    ? toolCall.input
    : JSON.stringify(toolCall.input);

  return (
    <div
      data-testid="assistant-tool-use-message"
      className={cn(
        'rounded-lg border px-3 py-2',
        isError && 'border-red-200 bg-red-50 dark:border-red-800 dark:bg-red-950/30',
        isRunning && !isResolved && !isError && 'border-blue-200 bg-blue-50 dark:border-blue-800 dark:bg-blue-950/30',
        isResolved && !isError && 'border-emerald-200 bg-emerald-50 dark:border-emerald-800 dark:bg-emerald-950/30',
        !isRunning && !isResolved && !isError && 'border-slate-200 bg-slate-50 dark:border-slate-700 dark:bg-slate-800/50',
        className,
      )}
    >
      <div className="flex items-center gap-2">
        {isRunning && !isResolved && (
          <Loader2 className="h-3.5 w-3.5 animate-spin text-blue-500" />
        )}
        {isResolved && !isError && (
          <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400" />
        )}
        {isError && (
          <XCircle className="h-3.5 w-3.5 text-red-600 dark:text-red-400" />
        )}
        {!isRunning && !isResolved && !isError && (
          <Wrench className="h-3.5 w-3.5 text-slate-500 dark:text-slate-400" />
        )}
        <span className="text-xs font-semibold text-slate-700 dark:text-slate-300">
          {toolCall.name}
        </span>
        {!verbose && (
          <span className="text-xs text-slate-500 dark:text-slate-400">
            {truncateMiddle(inputSummary, 60)}
          </span>
        )}
        {verbose && (
          <button
            type="button"
            onClick={toggleExpanded}
            className="flex items-center text-xs text-blue-600 hover:text-blue-800 dark:text-blue-400"
          >
            <ChevronRight
              className={cn(
                'h-3 w-3 transition-transform',
                expanded && 'rotate-90',
              )}
            />
            {expanded ? '收起' : '详情'}
          </button>
        )}
      </div>
      {progress && isRunning && (
        <div className="mt-1 text-xs text-blue-600 dark:text-blue-400">
          {progress.message}
        </div>
      )}
      {verbose && expanded && (
        <pre className="mt-2 overflow-x-auto whitespace-pre-wrap text-xs text-slate-600 dark:text-slate-400">
          {typeof toolCall.input === 'string'
            ? toolCall.input
            : JSON.stringify(toolCall.input, null, 2)}
        </pre>
      )}
    </div>
  );
});
