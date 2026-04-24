import { memo } from 'react';
import { cn } from '../../lib/utils';

/** 助手编辑思考消息组件属性 */
export interface AssistantRedactedThinkingMessageProps {
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 助手编辑思考占位符组件。
 * 显示 "Thinking redacted" 灰色斜体文本。
 */
export const AssistantRedactedThinkingMessage = memo(function AssistantRedactedThinkingMessage({
  className,
}: AssistantRedactedThinkingMessageProps) {
  return (
    <div
      data-testid="assistant-redacted-thinking"
      className={cn('italic text-slate-400 dark:text-slate-500', className)}
    >
      Thinking redacted
    </div>
  );
});
