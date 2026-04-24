/**
 * BackgroundTasksDialog — 后台任务列表对话框。
 *
 * 显示所有后台任务的列表，支持选择任务。
 * visible=false 时返回 null。
 */

import { X } from 'lucide-react';
import { cn } from '@/lib/utils';
import { BackgroundTask } from './BackgroundTask';

export interface TaskItem {
  id: string;
  name: string;
  status: string;
  progress?: number;
  startedAt: string;
}

export interface BackgroundTasksDialogProps {
  visible: boolean;
  tasks: TaskItem[];
  onClose: () => void;
  onSelectTask?: (id: string) => void;
  className?: string;
}

export function BackgroundTasksDialog({
  visible,
  tasks,
  onClose,
  onSelectTask,
  className,
}: BackgroundTasksDialogProps) {
  if (!visible) return null;

  return (
    <div
      data-testid="background-tasks-dialog"
      className={cn(
        'fixed inset-0 z-50 flex items-center justify-center bg-black/50',
        className,
      )}
    >
      <div className="w-full max-w-lg rounded-xl bg-white shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-200 px-4 py-3">
          <h2 className="text-sm font-semibold text-slate-900">后台任务</h2>
          <button
            onClick={onClose}
            className="rounded-lg p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
            data-testid="dialog-close"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Task list */}
        <div className="max-h-96 overflow-y-auto p-4">
          {tasks.length === 0 ? (
            <p className="py-8 text-center text-sm text-slate-400">
              暂无后台任务
            </p>
          ) : (
            <div className="space-y-2">
              {tasks.map((task) => (
                <BackgroundTask
                  key={task.id}
                  task={{
                    ...task,
                    status: task.status as 'running' | 'completed' | 'failed' | 'pending',
                  }}
                  onClick={onSelectTask ? () => onSelectTask(task.id) : undefined}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
