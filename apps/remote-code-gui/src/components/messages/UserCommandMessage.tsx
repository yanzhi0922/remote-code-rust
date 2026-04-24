import { memo } from 'react';
import { cn } from '../../lib/utils';

/** 用户命令消息组件属性 */
export interface UserCommandMessageProps {
  /** 命令名称 */
  command: string;
  /** 命令参数 */
  args?: string;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 用户命令消息渲染组件。
 * 显示 `/command args` 格式，带紫色样式。
 */
export const UserCommandMessage = memo(function UserCommandMessage({
  command,
  args,
  className,
}: UserCommandMessageProps) {
  return (
    <div
      data-testid="user-command-message"
      className={cn(
        'inline-flex items-center gap-1 rounded-md bg-violet-100 px-3 py-1.5 dark:bg-violet-950/40',
        className,
      )}
    >
      <span className="font-mono text-xs font-medium text-violet-700 dark:text-violet-400">
        /{command}
      </span>
      {args && (
        <span className="font-mono text-xs text-violet-500 dark:text-violet-300">
          {args}
        </span>
      )}
    </div>
  );
});
