import { memo } from 'react';
import { MessageSquare } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 上下文建议项 */
export interface ContextSuggestion {
  /** 建议标签 */
  label: string;
  /** 建议描述 */
  description: string;
}

/** 用户提示消息属性 */
export interface UserPromptMessageProps {
  /** 提示文本 */
  text: string;
  /** 上下文建议列表 */
  suggestions?: ContextSuggestion[];
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 用户提示消息组件。
 * 显示用户发送的提示，并附带上下文建议。
 */
export const UserPromptMessage = memo(function UserPromptMessage({
  text,
  suggestions,
  className,
}: UserPromptMessageProps) {
  if (!text.trim()) {
    return null;
  }

  return (
    <div className={cn('flex justify-end', className)}>
      <div className="max-w-3xl space-y-2">
        {/* 主提示气泡 */}
        <div className="rounded-[24px] bg-[#17181a] px-5 py-4 text-[15px] leading-7 text-white shadow-[0_14px_32px_rgba(23,24,26,0.16)] dark:bg-slate-700 dark:shadow-[0_14px_32px_rgba(0,0,0,0.3)]">
          <div className="whitespace-pre-wrap break-words">{text}</div>
        </div>

        {/* 上下文建议 */}
        {suggestions && suggestions.length > 0 && (
          <div className="flex flex-wrap justify-end gap-1.5">
            {suggestions.map((suggestion, index) => (
              <span
                key={`${suggestion.label}-${index}`}
                className="inline-flex items-center gap-1 rounded-full bg-slate-100 px-2.5 py-1 text-[11px] text-slate-600 dark:bg-slate-700 dark:text-slate-300"
              >
                <MessageSquare className="h-3 w-3" />
                <span className="font-medium">{suggestion.label}</span>
                {suggestion.description && (
                  <>
                    <span className="text-slate-400">·</span>
                    <span>{suggestion.description}</span>
                  </>
                )}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
});
