import { memo } from 'react';
import { ClipboardList } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 用户计划消息属性 */
export interface UserPlanMessageProps {
  /** 计划内容文本 */
  planContent: string;
  /** 是否添加外边距 */
  addMargin?: boolean;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 用户计划消息组件。
 * 在 plan mode 下显示用户确认的计划内容。
 */
export const UserPlanMessage = memo(function UserPlanMessage({
  planContent,
  addMargin = true,
  className,
}: UserPlanMessageProps) {
  if (!planContent.trim()) {
    return null;
  }

  return (
    <div
      className={cn(
        'rounded-2xl border border-blue-200 bg-blue-50/80 px-5 py-4 dark:border-blue-800 dark:bg-blue-950/30',
        addMargin && 'my-2',
        className,
      )}
    >
      <div className="mb-2 flex items-center gap-2">
        <ClipboardList className="h-4 w-4 text-blue-600 dark:text-blue-400" />
        <span className="text-xs font-semibold uppercase tracking-wider text-blue-700 dark:text-blue-400">
          用户计划
        </span>
      </div>
      <div className="whitespace-pre-wrap text-sm leading-6 text-slate-700 dark:text-slate-300">
        {planContent}
      </div>
    </div>
  );
});
