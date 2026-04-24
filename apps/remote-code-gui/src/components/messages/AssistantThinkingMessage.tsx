import { memo, useState, useCallback } from 'react';
import { Brain, ChevronRight } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 助手思考消息组件属性 */
export interface AssistantThinkingMessageProps {
  /** 思考内容文本 */
  thinking: string;
  /** 是否为转录模式 */
  isTranscriptMode?: boolean;
  /** 是否显示详细信息 */
  verbose?: boolean;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 助手思考消息渲染组件。
 * 从 content_blocks 中 type === 'thinking' 的块提取内容。
 * 非详细模式显示折叠行，详细模式/transcript 模式显示完整思考内容。
 */
export const AssistantThinkingMessage = memo(function AssistantThinkingMessage({
  thinking,
  isTranscriptMode = false,
  verbose = false,
  className,
}: AssistantThinkingMessageProps) {
  const [expanded, setExpanded] = useState(false);
  const toggleExpanded = useCallback(() => setExpanded((prev) => !prev), []);

  if (!thinking.trim()) {
    return null;
  }

  const showFull = verbose || isTranscriptMode || expanded;

  return (
    <div
      data-testid="assistant-thinking-message"
      className={cn('rounded-lg border border-slate-200 bg-slate-50 px-4 py-3 dark:border-slate-700 dark:bg-slate-800/50', className)}
    >
      <button
        type="button"
        onClick={toggleExpanded}
        className="flex items-center gap-2 text-xs font-medium text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-300"
      >
        <Brain className="h-3.5 w-3.5" />
        <span>∴ Thinking</span>
        {!verbose && !isTranscriptMode && (
          <ChevronRight
            className={cn(
              'h-3 w-3 transition-transform',
              expanded && 'rotate-90',
            )}
          />
        )}
      </button>
      {showFull && (
        <div className="mt-2 whitespace-pre-wrap text-xs leading-5 text-slate-600 dark:text-slate-400">
          {thinking}
        </div>
      )}
    </div>
  );
});
