import { memo } from 'react';
import { Power, XCircle, CheckCircle2 } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 关机消息组件属性 */
export interface ShutdownMessageProps {
  /** 消息变体 */
  variant: 'request' | 'rejected' | 'approved';
  /** 发起者 */
  from: string;
  /** 原因 */
  reason?: string;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 关机消息渲染组件。
 * 根据变体显示不同颜色和图标。
 */
export const ShutdownMessage = memo(function ShutdownMessage({
  variant,
  from,
  reason,
  className,
}: ShutdownMessageProps) {
  return (
    <div
      data-testid="shutdown-message"
      className={cn(
        'rounded-lg border px-4 py-3',
        variant === 'request' && 'border-amber-300 bg-amber-50 dark:border-amber-800 dark:bg-amber-950/30',
        variant === 'rejected' && 'border-slate-300 bg-slate-50 dark:border-slate-600 dark:bg-slate-800/50',
        variant === 'approved' && 'border-emerald-300 bg-emerald-50 dark:border-emerald-800 dark:bg-emerald-950/30',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        {variant === 'request' && (
          <Power className="mt-0.5 h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400" />
        )}
        {variant === 'rejected' && (
          <XCircle className="mt-0.5 h-4 w-4 shrink-0 text-slate-500 dark:text-slate-400" />
        )}
        {variant === 'approved' && (
          <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-emerald-600 dark:text-emerald-400" />
        )}
        <div>
          <p className="text-xs font-medium text-slate-700 dark:text-slate-300">
            {variant === 'request' && `Shutdown request from ${from}`}
            {variant === 'rejected' && `Shutdown rejected by ${from}`}
            {variant === 'approved' && `Shutdown approved by ${from}`}
          </p>
          {reason && (
            <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
              {reason}
            </p>
          )}
        </div>
      </div>
    </div>
  );
});
