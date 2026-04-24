import { memo } from 'react';
import { Loader2 } from 'lucide-react';
import { cn } from '../../lib/utils';

/** Hook 进度消息组件属性 */
export interface HookProgressMessageProps {
  /** Hook 事件名称 */
  hookEvent: string;
  /** 进行中的数量 */
  inProgressCount: number;
  /** 已完成的数量 */
  resolvedCount: number;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * Hook 进度消息渲染组件。
 * 显示 "3 PreToolUse hooks ran" 格式的进度信息。
 * inProgressCount === 0 时返回 null。
 */
export const HookProgressMessage = memo(function HookProgressMessage({
  hookEvent,
  inProgressCount,
  resolvedCount,
  className,
}: HookProgressMessageProps) {
  if (inProgressCount === 0) {
    return null;
  }

  const total = inProgressCount + resolvedCount;

  return (
    <div
      data-testid="hook-progress-message"
      className={cn(
        'flex items-center gap-2 rounded-md bg-slate-100 px-3 py-1.5 text-xs text-slate-600 dark:bg-slate-800 dark:text-slate-400',
        className,
      )}
    >
      <Loader2 className="h-3 w-3 animate-spin" />
      <span>
        {total} {hookEvent} hooks ran
      </span>
    </div>
  );
});
