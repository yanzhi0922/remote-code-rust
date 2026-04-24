import { memo, useMemo } from 'react';
import { cn } from '../lib/utils';

/** 消息时间戳属性 */
export interface MessageTimestampProps {
  /** ISO 8601 时间戳字符串 */
  timestamp: string | null;
  /** 是否为 transcript 模式（始终显示） */
  isTranscriptMode?: boolean;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 消息时间戳显示组件。
 * 在 transcript 模式下始终显示，否则仅悬停时可见。
 */
export const MessageTimestamp = memo(function MessageTimestamp({
  timestamp,
  isTranscriptMode = false,
  className,
}: MessageTimestampProps) {
  const formattedTime = useMemo(() => {
    if (!timestamp) return null;
    try {
      return new Date(timestamp).toLocaleTimeString('zh-CN', {
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
      });
    } catch {
      return null;
    }
  }, [timestamp]);

  if (!formattedTime) {
    return null;
  }

  return (
    <span
      className={cn(
        'shrink-0 text-[11px] tabular-nums text-slate-400 dark:text-slate-500',
        isTranscriptMode ? 'inline-block' : 'opacity-0 transition-opacity group-hover:opacity-100',
        className,
      )}
      title={timestamp ?? undefined}
    >
      {formattedTime}
    </span>
  );
});
