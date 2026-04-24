import { memo, useState, useCallback } from 'react';
import { AlertCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 系统 API 错误消息组件属性 */
export interface SystemAPIErrorMessageProps {
  /** 错误消息文本 */
  message: string;
  /** HTTP 状态码 */
  statusCode?: number;
  /** 额外的 CSS 类名 */
  className?: string;
}

/** 截断阈值 */
const TRUNCATE_LENGTH = 500;

/**
 * 系统 API 错误消息渲染组件。
 * 显示 API 错误信息，带红色边框和可选状态码标签。
 */
export const SystemAPIErrorMessage = memo(function SystemAPIErrorMessage({
  message,
  statusCode,
  className,
}: SystemAPIErrorMessageProps) {
  const [expanded, setExpanded] = useState(false);
  const toggleExpanded = useCallback(() => setExpanded((prev) => !prev), []);

  const isLong = message.length > TRUNCATE_LENGTH;
  const displayMessage = isLong && !expanded
    ? message.slice(0, TRUNCATE_LENGTH) + '…'
    : message;

  return (
    <div
      data-testid="system-api-error"
      className={cn(
        'rounded-lg border border-red-300 bg-red-50 px-4 py-3 dark:border-red-800 dark:bg-red-950/30',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-red-600 dark:text-red-400" />
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <span className="text-xs font-semibold uppercase tracking-wider text-red-700 dark:text-red-400">
              API Error
            </span>
            {statusCode != null && (
              <span className="rounded-full bg-red-100 px-2 py-0.5 text-[11px] font-medium text-red-700 dark:bg-red-900 dark:text-red-300">
                {statusCode}
              </span>
            )}
          </div>
          <pre className="whitespace-pre-wrap break-words text-xs leading-5 text-red-700 dark:text-red-300">
            {displayMessage}
          </pre>
          {isLong && (
            <button
              type="button"
              onClick={toggleExpanded}
              className="mt-1 text-xs text-red-600 hover:text-red-800 dark:text-red-400 dark:hover:text-red-300"
            >
              {expanded ? '收起' : '展开全部'}
            </button>
          )}
        </div>
      </div>
    </div>
  );
});
