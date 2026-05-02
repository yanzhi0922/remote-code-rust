import { cn } from '../../lib/utils';

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export interface TaskItemData {
  name: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  activeForm?: string;
}

export interface TaskTreeProps {
  toolCalls?: TaskItemData[];
}

export function TaskTree({ toolCalls = [] }: TaskTreeProps) {
  if (toolCalls.length === 0) {
    return null;
  }

  const completed = toolCalls.filter((t) => t.status === 'completed').length;

  return (
    <div className="w-full border border-[#eaeaea] rounded-[10px] bg-white shadow-[0_8px_30px_rgb(0,0,0,0.08)]">
      {/* Task Header */}
      <div className="flex items-center justify-between px-5 py-3 border-b border-[#f5f5f5]">
        <div className="flex items-center gap-2 text-[13px] font-medium text-slate-800">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-slate-400"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>
          <span>共 {toolCalls.length} 个任务，已完成 {completed} 个</span>
        </div>
      </div>

      {/* Task Items */}
      <div className="p-2 space-y-0.5">
        {toolCalls.map((task, idx) => (
          <TaskItem
            key={idx}
            number={idx + 1}
            title={task.name}
            activeForm={task.activeForm}
            active={task.status === 'running'}
            disabled={task.status === 'pending'}
            completed={task.status === 'completed'}
            failed={task.status === 'failed'}
          />
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Internal components
// ---------------------------------------------------------------------------

function TaskItem({
  number,
  title,
  activeForm,
  active,
  disabled,
  completed,
  failed,
}: {
  number: number;
  title: string;
  activeForm?: string;
  active?: boolean;
  disabled?: boolean;
  completed?: boolean;
  failed?: boolean;
}) {
  return (
    <div
      className={cn(
        'flex items-start gap-4 px-4 py-2.5 rounded-lg transition-colors text-[13px] font-medium',
        active ? 'bg-[#f4f3ec] text-slate-800' : '',
        completed ? 'text-emerald-700' : '',
        failed ? 'text-red-600' : '',
        disabled && !completed && !failed ? 'opacity-40 text-slate-400' : 'text-slate-600 hover:bg-[#f8f8f8]',
      )}
    >
      <div className="mt-0.5 shrink-0 flex items-center gap-2">
        <div
          className={cn(
            'w-3 h-3 rounded-full border-2',
            completed ? 'border-emerald-500 bg-emerald-500' : '',
            failed ? 'border-red-500 bg-red-500' : '',
            active ? 'border-blue-500 animate-pulse' : '',
            !completed && !failed && !active ? 'border-slate-300' : '',
          )}
        />
        <span className="text-slate-400 font-bold ml-1">{number}.</span>
      </div>
      <div className="flex gap-2 w-full">
        <span className="leading-5">{active && activeForm ? activeForm : title}</span>
        {completed && (
          <span className="text-[10px] text-emerald-500 font-semibold self-center">✓</span>
        )}
        {failed && (
          <span className="text-[10px] text-red-500 font-semibold self-center">✗</span>
        )}
        {active && (
          <span className="text-[10px] text-blue-500 font-semibold self-center">运行中</span>
        )}
      </div>
    </div>
  );
}

