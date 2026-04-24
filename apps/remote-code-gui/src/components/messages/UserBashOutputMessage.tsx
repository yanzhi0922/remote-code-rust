import { memo } from 'react';
import { cn } from '../../lib/utils';

/** 用户 Bash 输出消息组件属性 */
export interface UserBashOutputMessageProps {
  /** 输出文本 */
  output: string;
  /** 输出流 */
  stream?: 'stdout' | 'stderr';
  /** 退出码 */
  exitCode?: number;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 用户 Bash 输出消息渲染组件。
 * monospace 字体，stderr 红色，exitCode !== 0 时显示退出码。
 */
export const UserBashOutputMessage = memo(function UserBashOutputMessage({
  output,
  stream = 'stdout',
  exitCode,
  className,
}: UserBashOutputMessageProps) {
  const isStderr = stream === 'stderr';
  const hasError = exitCode != null && exitCode !== 0;

  return (
    <div
      data-testid="user-bash-output"
      className={cn(
        'rounded-md bg-slate-50 px-3 py-2 dark:bg-slate-800/50',
        isStderr && 'bg-red-50 dark:bg-red-950/20',
        className,
      )}
    >
      <pre
        className={cn(
          'overflow-x-auto whitespace-pre-wrap break-words font-mono text-xs leading-5',
          isStderr
            ? 'text-red-700 dark:text-red-400'
            : 'text-slate-700 dark:text-slate-300',
        )}
      >
        {output}
      </pre>
      {hasError && (
        <div className="mt-1 text-xs text-red-600 dark:text-red-400">
          Exit code: {exitCode}
        </div>
      )}
    </div>
  );
});
