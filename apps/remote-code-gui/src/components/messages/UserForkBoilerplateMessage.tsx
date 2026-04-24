import { GitFork } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface UserForkBoilerplateMessageProps {
  templateName: string;
  targetPath?: string;
  status?: 'pending' | 'completed' | 'failed';
  className?: string;
}

export function UserForkBoilerplateMessage({
  templateName,
  targetPath,
  status = 'pending',
  className,
}: UserForkBoilerplateMessageProps) {
  const statusColors: Record<string, string> = {
    pending: 'text-slate-500',
    completed: 'text-green-600 dark:text-green-400',
    failed: 'text-red-600 dark:text-red-400',
  };

  return (
    <div
      className={cn(
        'flex items-start gap-2 rounded-lg border border-slate-200 bg-slate-50 px-4 py-3 dark:border-slate-700 dark:bg-slate-800/50',
        className,
      )}
      data-testid="user-fork-boilerplate-message"
    >
      <GitFork className="mt-0.5 h-4 w-4 shrink-0 text-slate-400" />
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-slate-700 dark:text-slate-300">
          Fork 模板: {templateName}
        </p>
        {targetPath && (
          <p className="mt-0.5 truncate font-mono text-xs text-slate-500">
            → {targetPath}
          </p>
        )}
        <span className={cn('text-xs', statusColors[status])}>
          {status === 'pending' && '等待中'}
          {status === 'completed' && '已完成'}
          {status === 'failed' && '失败'}
        </span>
      </div>
    </div>
  );
}
