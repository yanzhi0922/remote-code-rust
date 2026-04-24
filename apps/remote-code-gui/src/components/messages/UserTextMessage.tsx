import { memo, lazy, Suspense } from 'react';
import type { ConversationEntry } from '../../lib/types';
import { cn } from '../../lib/utils';

const LazyMarkdownRenderer = lazy(() => import('../chat/MarkdownRenderer'));

/** 用户文本消息组件的属性 */
export interface UserTextMessageProps {
  /** 对话条目 */
  entry: ConversationEntry;
  /** 是否显示时间戳 */
  showTimestamp?: boolean;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 用户文本消息渲染组件。
 * 显示用户发送的纯文本消息，深色气泡靠右对齐。
 */
export const UserTextMessage = memo(function UserTextMessage({
  entry,
  className,
}: UserTextMessageProps) {
  if (!entry.text.trim()) {
    return null;
  }

  return (
    <div className={cn('flex justify-end', className)}>
      <div className="max-w-3xl rounded-[24px] bg-[#17181a] px-5 py-4 text-[15px] leading-7 text-white shadow-[0_14px_32px_rgba(23,24,26,0.16)] dark:bg-slate-700 dark:shadow-[0_14px_32px_rgba(0,0,0,0.3)]">
        <div className="whitespace-pre-wrap break-words">
          <Suspense fallback={<span>{entry.text}</span>}>
            <LazyMarkdownRenderer content={entry.text} />
          </Suspense>
        </div>
      </div>
    </div>
  );
});
