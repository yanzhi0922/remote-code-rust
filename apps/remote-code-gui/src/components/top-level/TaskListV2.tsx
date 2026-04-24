import { type ReactNode } from 'react';
import { CheckCircle2, Circle, Loader2 } from 'lucide-react';
import { cn } from '../../lib/utils';

type TaskStatus = 'pending' | 'in_progress' | 'completed';

interface TaskItem {
  id: string;
  title: string;
  status: TaskStatus;
  owner?: string;
  blockedBy: string[];
}

interface Props {
  tasks: TaskItem[];
  isStandalone?: boolean;
}

function StatusIcon({ status }: { status: TaskStatus }): ReactNode {
  switch (status) {
    case 'completed':
      return <CheckCircle2 className="h-4 w-4 text-green-500" />;
    case 'in_progress':
      return <Loader2 className="h-4 w-4 animate-spin text-blue-500" />;
    case 'pending':
      return <Circle className="h-4 w-4 text-gray-400" />;
  }
}

export function TaskListV2({ tasks, isStandalone = false }: Props): ReactNode {
  if (tasks.length === 0) return null;

  const completedCount = tasks.filter((t) => t.status === 'completed').length;
  const pendingCount = tasks.filter((t) => t.status === 'pending').length;
  const inProgressCount = tasks.length - completedCount - pendingCount;

  const content = (
    <div data-testid="task-list-v2" className="flex flex-col gap-0.5">
      {tasks.map((task) => (
        <div
          key={task.id}
          data-testid={`task-item-${task.id}`}
          className={cn(
            'flex items-center gap-2 rounded px-2 py-1 text-sm',
            task.status === 'completed' && 'opacity-60',
          )}
        >
          <StatusIcon status={task.status} />
          <span className="flex-1 truncate text-gray-800 dark:text-gray-200">{task.title}</span>
          {task.owner && (
            <span className="text-xs text-gray-500 dark:text-gray-400">@{task.owner}</span>
          )}
        </div>
      ))}
    </div>
  );

  if (isStandalone) {
    return (
      <div className="ml-4 mt-2 flex flex-col gap-1">
        <div className="text-xs text-gray-500 dark:text-gray-400">
          <span className="font-semibold">{tasks.length}</span> tasks ({' '}
          <span className="font-semibold">{completedCount}</span> done
          {inProgressCount > 0 && (
            <>
              , <span className="font-semibold">{inProgressCount}</span> in progress
            </>
          )}
          {pendingCount > 0 && (
            <>
              , <span className="font-semibold">{pendingCount}</span> pending
            </>
          )}
          )
        </div>
        {content}
      </div>
    );
  }

  return <div className="flex flex-col">{content}</div>;
}
