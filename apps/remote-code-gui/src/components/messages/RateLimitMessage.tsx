import { memo } from 'react';
import { AlertTriangle, Zap } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 速率限制消息组件属性 */
export interface RateLimitMessageProps {
  /** 限制消息文本 */
  text: string;
  /** 升级回调 */
  onUpgrade?: () => void;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 速率限制消息渲染组件。
 * 显示速率限制警告，带黄色/橙色样式和可选升级按钮。
 */
export const RateLimitMessage = memo(function RateLimitMessage({
  text,
  onUpgrade,
  className,
}: RateLimitMessageProps) {
  return (
    <div
      data-testid="rate-limit-message"
      className={cn(
        'rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 dark:border-amber-800 dark:bg-amber-950/30',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400" />
        <div className="min-w-0 flex-1">
          <p className="text-xs leading-5 text-amber-800 dark:text-amber-300">
            {text}
          </p>
          {onUpgrade && (
            <button
              type="button"
              onClick={onUpgrade}
              className="mt-2 inline-flex items-center gap-1 rounded-md bg-amber-600 px-3 py-1 text-xs font-medium text-white hover:bg-amber-700 dark:bg-amber-700 dark:hover:bg-amber-600"
            >
              <Zap className="h-3 w-3" />
              Upgrade
            </button>
          )}
        </div>
      </div>
    </div>
  );
});
