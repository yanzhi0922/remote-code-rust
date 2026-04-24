import { memo, useState, useCallback } from 'react';
import { ChevronRight, AlertTriangle } from 'lucide-react';
import type { ConversationEntry } from '../../lib/types';
import { cn } from '../../lib/utils';
import { Markdown } from './Markdown';

/** 助手文本消息组件属性 */
export interface AssistantTextMessageProps {
  /** 对话条目 */
  entry: ConversationEntry;
  /** 是否为转录模式 */
  isTranscriptMode?: boolean;
  /** 是否显示详细信息 */
  verbose?: boolean;
  /** 额外的 CSS 类名 */
  className?: string;
}

/** 长文本折叠阈值（字符数） */
const COLLAPSE_THRESHOLD = 600;

/**
 * 检测文本是否包含特殊错误模式。
 */
function detectErrorKind(text: string): 'rate-limit' | 'api-error' | 'timeout' | null {
  const lower = text.toLowerCase();
  if (lower.includes('rate limit') || lower.includes('rate_limit') || lower.includes('too many requests')) {
    return 'rate-limit';
  }
  if (lower.includes('api error') || lower.includes('api_error') || lower.includes('internal server error')) {
    return 'api-error';
  }
  if (lower.includes('timeout') || lower.includes('timed out') || lower.includes('deadline exceeded')) {
    return 'timeout';
  }
  return null;
}

/**
 * 助手文本消息渲染组件。
 * 显示助手回复的文本内容，支持 Markdown 格式和展开/折叠。
 */
export const AssistantTextMessage = memo(function AssistantTextMessage({
  entry,
  isTranscriptMode = false,
  verbose = false,
  className,
}: AssistantTextMessageProps) {
  const [expanded, setExpanded] = useState(false);
  const toggleExpanded = useCallback(() => setExpanded((prev) => !prev), []);

  if (!entry.text.trim()) {
    return null;
  }

  const errorKind = detectErrorKind(entry.text);
  const isLong = entry.text.length > COLLAPSE_THRESHOLD;
  const shouldCollapse = isLong && !expanded && !isTranscriptMode && !verbose;

  const displayText = shouldCollapse
    ? entry.text.slice(0, COLLAPSE_THRESHOLD) + '…'
    : entry.text;

  return (
    <div
      data-testid="assistant-text-message"
      className={cn(
        'max-w-4xl',
        errorKind && 'rounded-lg border border-red-200 bg-red-50 px-4 py-3 dark:border-red-800 dark:bg-red-950/30',
        !errorKind && 'text-slate-800 dark:text-slate-200',
        className,
      )}
    >
      {errorKind && (
        <div className="mb-2 flex items-center gap-1.5 text-red-600 dark:text-red-400">
          <AlertTriangle className="h-4 w-4" />
          <span className="text-xs font-semibold uppercase tracking-wider">
            {errorKind === 'rate-limit' && 'Rate Limit'}
            {errorKind === 'api-error' && 'API Error'}
            {errorKind === 'timeout' && 'Timeout'}
          </span>
        </div>
      )}
      <Markdown>{displayText}</Markdown>
      {isLong && !isTranscriptMode && (
        <button
          type="button"
          onClick={toggleExpanded}
          className="mt-2 flex items-center gap-1 text-xs text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300"
        >
          <ChevronRight
            className={cn(
              'h-3 w-3 transition-transform',
              expanded && 'rotate-90',
            )}
          />
          {expanded ? '收起' : '展开全部'}
        </button>
      )}
    </div>
  );
});
