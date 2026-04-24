import { memo } from 'react';
import { Users } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 团队成员消息属性 */
export interface UserTeammateMessageProps {
  /** 消息文本 */
  text: string;
  /** 发送者名称 */
  senderName: string;
  /** 发送者角色（如 leader、worker） */
  senderRole?: string;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 团队成员消息组件。
 * 显示来自协作团队成员的消息，带有成员标识。
 */
export const UserTeammateMessage = memo(function UserTeammateMessage({
  text,
  senderName,
  senderRole,
  className,
}: UserTeammateMessageProps) {
  if (!text.trim()) {
    return null;
  }

  return (
    <div
      className={cn(
        'rounded-2xl border border-indigo-200 bg-indigo-50/60 px-5 py-4 dark:border-indigo-800 dark:bg-indigo-950/20',
        className,
      )}
    >
      <div className="mb-2 flex items-center gap-2">
        <Users className="h-4 w-4 text-indigo-600 dark:text-indigo-400" />
        <span className="text-sm font-semibold text-indigo-700 dark:text-indigo-300">
          {senderName}
        </span>
        {senderRole && (
          <span className="rounded-full bg-indigo-100 px-2 py-0.5 text-[11px] text-indigo-600 dark:bg-indigo-900 dark:text-indigo-400">
            {senderRole}
          </span>
        )}
      </div>
      <div className="whitespace-pre-wrap text-sm leading-6 text-slate-700 dark:text-slate-300">
        {text}
      </div>
    </div>
  );
});
