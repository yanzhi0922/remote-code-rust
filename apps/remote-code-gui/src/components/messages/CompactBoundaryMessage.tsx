import { memo } from 'react';
import { cn } from '../../lib/utils';

/** 压缩边界消息组件属性 */
export interface CompactBoundaryMessageProps {
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 压缩边界消息渲染组件。
 * 显示 "✻ Conversation compacted" 灰色分隔线。
 */
export const CompactBoundaryMessage = memo(function CompactBoundaryMessage({
  className,
}: CompactBoundaryMessageProps) {
  return (
    <div
      data-testid="compact-boundary"
      className={cn(
        'flex items-center gap-3 py-2 text-xs text-slate-400 dark:text-slate-500',
        className,
      )}
    >
      <div className="h-px flex-1 bg-slate-200 dark:bg-slate-700" />
      <span className="shrink-0">✻ Conversation compacted</span>
      <div className="h-px flex-1 bg-slate-200 dark:bg-slate-700" />
    </div>
  );
});
