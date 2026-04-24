import { type ReactNode } from 'react';
import { Play, Pause, Circle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface CoordinatorTask {
  id: string;
  name?: string;
  status: 'pending' | 'in_progress' | 'completed';
  description: string;
  startTime: number;
  endTime?: number;
  tokenCount?: number;
}

interface Props {
  tasks: CoordinatorTask[];
  viewingTaskId?: string;
  selectedIndex?: number;
  onTaskClick?: (taskId: string) => void;
  onMainClick?: () => void;
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes}m ${remainingSeconds}s`;
}

export function CoordinatorAgentStatus({
  tasks,
  viewingTaskId,
  selectedIndex,
  onTaskClick,
  onMainClick,
}: Props): ReactNode {
  if (tasks.length === 0) return null;

  return (
    <div data-testid="coordinator-agent-status" className="mt-2 flex flex-col">
      <button
        data-testid="coordinator-main"
        onClick={onMainClick}
        className={cn(
          'flex items-center gap-2 rounded px-2 py-1 text-sm',
          viewingTaskId === undefined ? 'font-semibold text-gray-900 dark:text-gray-100' : 'text-gray-600 dark:text-gray-400',
          selectedIndex === 0 && 'bg-gray-100 dark:bg-gray-700',
        )}
      >
        <Circle className={cn('h-3 w-3', viewingTaskId === undefined ? 'fill-current' : '')} />
        <span>main</span>
      </button>

      {tasks.map((task, i) => {
        const isRunning = task.status === 'in_progress';
        const isViewed = viewingTaskId === task.id;
        const elapsed = (task.endTime ?? Date.now()) - task.startTime;

        return (
          <button
            key={task.id}
            data-testid={`coordinator-task-${task.id}`}
            onClick={() => onTaskClick?.(task.id)}
            className={cn(
              'flex items-center gap-2 rounded px-2 py-1 text-sm',
              isViewed ? 'font-semibold text-gray-900 dark:text-gray-100' : 'text-gray-600 dark:text-gray-400',
              selectedIndex === i + 1 && 'bg-gray-100 dark:bg-gray-700',
            )}
          >
            {isRunning ? (
              <Play className="h-3 w-3 text-green-500" />
            ) : (
              <Pause className="h-3 w-3 text-gray-400" />
            )}
            <span className="flex-1 truncate">
              {task.name ? `${task.name}: ` : ''}{task.description}
            </span>
            <span className="shrink-0 text-xs text-gray-400">
              {formatDuration(elapsed)}
              {task.tokenCount != null && task.tokenCount > 0 && (
                <> · {task.tokenCount} tokens</>
              )}
            </span>
          </button>
        );
      })}
    </div>
  );
}
