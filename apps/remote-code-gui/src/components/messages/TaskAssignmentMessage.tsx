import { memo } from 'react';
import { ClipboardList } from 'lucide-react';
import { cn } from '../../lib/utils';

/** 任务分配消息组件属性 */
export interface TaskAssignmentMessageProps {
  /** 任务 ID */
  taskId: string;
  /** 分配者 */
  assignedBy: string;
  /** 任务主题 */
  subject: string;
  /** 任务描述 */
  description?: string;
  /** 额外的 CSS 类名 */
  className?: string;
}

/**
 * 任务分配消息渲染组件。
 * 显示青色边框的任务分配信息。
 */
export const TaskAssignmentMessage = memo(function TaskAssignmentMessage({
  taskId,
  assignedBy,
  subject,
  description,
  className,
}: TaskAssignmentMessageProps) {
  return (
    <div
      data-testid="task-assignment-message"
      className={cn(
        'rounded-lg border border-cyan-300 bg-cyan-50 px-4 py-3 dark:border-cyan-800 dark:bg-cyan-950/30',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        <ClipboardList className="mt-0.5 h-4 w-4 shrink-0 text-cyan-600 dark:text-cyan-400" />
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <span className="text-xs font-semibold text-cyan-700 dark:text-cyan-400">
              Task Assigned
            </span>
            <span className="rounded-full bg-cyan-100 px-2 py-0.5 text-[11px] font-medium text-cyan-700 dark:bg-cyan-900 dark:text-cyan-300">
              {taskId}
            </span>
          </div>
          <p className="text-xs font-medium text-slate-700 dark:text-slate-300">
            {subject}
          </p>
          <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
            Assigned by {assignedBy}
          </p>
          {description && (
            <p className="mt-2 text-xs leading-5 text-slate-600 dark:text-slate-400">
              {description}
            </p>
          )}
        </div>
      </div>
    </div>
  );
});
