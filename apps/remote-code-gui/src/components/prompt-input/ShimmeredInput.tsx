import { cn } from '../../lib/utils';

/** ShimmeredInput 组件属性 */
export interface ShimmeredInputProps {
  /** 额外 CSS 类名 */
  className?: string;
}

/**
 * 加载中的输入框占位符。
 * 使用 CSS 动画实现闪烁效果。
 */
export function ShimmeredInput({ className }: ShimmeredInputProps) {
  return (
    <div
      className={cn(
        'relative h-10 w-full overflow-hidden rounded-lg border border-slate-200 bg-slate-100 dark:border-slate-700 dark:bg-slate-800',
        className,
      )}
      data-testid="shimmered-input"
    >
      <div className="absolute inset-0 -translate-x-full animate-[shimmer_2s_infinite] bg-gradient-to-r from-transparent via-white/40 to-transparent dark:via-slate-600/20" />
    </div>
  );
}
