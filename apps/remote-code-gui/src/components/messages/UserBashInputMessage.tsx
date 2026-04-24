import { memo } from 'react';
import { Terminal } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 用户 Bash 输入消息组件属性 */
export interface UserBashInputMessageProps {
  /** 命令文本 */
  command: string;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 用户 Bash 输入消息渲染组件。
 * 显示 `$` 前缀 + 命令文本，monospace 字体。
 */
export const UserBashInputMessage = memo(function UserBashInputMessage({
  command,
  className,
}: UserBashInputMessageProps) {
  return (
    <div
      data-testid="user-bash-input"
      className={cn(
        'flex items-start gap-2 rounded-md bg-slate-100 px-3 py-2 dark:bg-slate-800',
        className,
      )}
    >
      <Terminal className="mt-0.5 h-3.5 w-3.5 shrink-0 text-slate-500 dark:text-slate-400" />
      <code className="whitespace-pre-wrap break-words font-mono text-xs leading-5 text-slate-800 dark:text-slate-200">
        <span className="text-emerald-600 dark:text-emerald-400">$ </span>
        {command}
      </code>
    </div>
  );
});
