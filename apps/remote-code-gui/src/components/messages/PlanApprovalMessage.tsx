import { memo } from 'react';
import { FileText, CheckCircle2, XCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 计划审批消息组件属性 */
export interface PlanApprovalMessageProps {
  /** 消息变体 */
  variant: 'request' | 'response';
  /** 发起者 */
  from: string;
  /** 计划内容 */
  planContent?: string;
  /** 计划文件路径 */
  planFilePath?: string;
  /** 是否批准 */
  approved?: boolean;
  /** 原因 */
  reason?: string;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 计划审批消息渲染组件。
 * request 变体显示蓝色虚线边框和计划内容。
 * response 变体显示批准/拒绝状态。
 */
export const PlanApprovalMessage = memo(function PlanApprovalMessage({
  variant,
  from,
  planContent,
  planFilePath,
  approved,
  reason,
  className,
}: PlanApprovalMessageProps) {
  return (
    <div
      data-testid="plan-approval-message"
      className={cn(
        'rounded-lg border px-4 py-3',
        variant === 'request' && 'border-dashed border-blue-300 bg-blue-50 dark:border-blue-800 dark:bg-blue-950/30',
        variant === 'response' && approved && 'border-emerald-300 bg-emerald-50 dark:border-emerald-800 dark:bg-emerald-950/30',
        variant === 'response' && !approved && 'border-red-300 bg-red-50 dark:border-red-800 dark:bg-red-950/30',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        {variant === 'request' && (
          <FileText className="mt-0.5 h-4 w-4 shrink-0 text-blue-600 dark:text-blue-400" />
        )}
        {variant === 'response' && approved && (
          <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-emerald-600 dark:text-emerald-400" />
        )}
        {variant === 'response' && !approved && (
          <XCircle className="mt-0.5 h-4 w-4 shrink-0 text-red-600 dark:text-red-400" />
        )}
        <div className="min-w-0 flex-1">
          {variant === 'request' && (
            <>
              <p className="text-xs font-medium text-blue-700 dark:text-blue-400">
                Plan approval request from {from}
              </p>
              {planFilePath && (
                <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                  {planFilePath}
                </p>
              )}
              {planContent && (
                <pre className="mt-2 whitespace-pre-wrap text-xs leading-5 text-slate-600 dark:text-slate-400">
                  {planContent}
                </pre>
              )}
            </>
          )}
          {variant === 'response' && (
            <>
              <p className="text-xs font-medium text-slate-700 dark:text-slate-300">
                {approved ? `Plan approved by ${from}` : `Plan rejected by ${from}`}
              </p>
              {reason && (
                <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                  {reason}
                </p>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
});
