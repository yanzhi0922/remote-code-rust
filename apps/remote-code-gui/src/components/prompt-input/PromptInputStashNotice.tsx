import { cn } from '../../lib/utils';

/** PromptInputStashNotice 组件属性 */
export interface PromptInputStashNoticeProps {
  /** 是否有暂存输入 */
  hasStashedInput: boolean;
  /** 恢复暂存输入回调 */
  onRestore: () => void;
  /** 额外 CSS 类名 */
  className?: string;
}

/**
 * 暂存输入提示。
 * 当有暂存输入时显示恢复提示，否则返回 null。
 */
export function PromptInputStashNotice({
  hasStashedInput,
  onRestore,
  className,
}: PromptInputStashNoticeProps) {
  if (!hasStashedInput) return null;

  return (
    <div
      className={cn(
        'flex items-center gap-2 rounded-md bg-indigo-50 px-3 py-1.5 text-xs text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-300',
        className,
      )}
      data-testid="prompt-stash-notice"
    >
      <span>You have stashed input. Press Ctrl+Shift+U to restore.</span>
      <button
        type="button"
        onClick={onRestore}
        className="font-medium underline hover:text-indigo-900 dark:hover:text-indigo-100"
      >
        Restore
      </button>
    </div>
  );
}
