import { ChevronRight, Users } from 'lucide-react';
import { useState } from 'react';
import { cn } from '../../lib/utils';

export interface TeamMemCollapsedProps {
  members: string[];
  maxVisible?: number;
  className?: string;
}

export function TeamMemCollapsed({
  members,
  maxVisible = 3,
  className,
}: TeamMemCollapsedProps) {
  const [expanded, setExpanded] = useState(false);
  const visible = expanded ? members : members.slice(0, maxVisible);
  const hidden = members.length - maxVisible;

  return (
    <div
      className={cn(
        'flex items-center gap-1.5 rounded-md bg-slate-100 px-2 py-1 text-xs text-slate-600 dark:bg-slate-800 dark:text-slate-400',
        className,
      )}
      data-testid="team-mem-collapsed"
    >
      <Users className="h-3.5 w-3.5 shrink-0" />
      <span className="font-medium">{members.length} 成员</span>
      <span className="text-slate-400">·</span>
      {visible.map((m) => (
        <span key={m} className="rounded bg-slate-200 px-1.5 py-0.5 dark:bg-slate-700">
          {m}
        </span>
      ))}
      {!expanded && hidden > 0 && (
        <button
          className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-300"
          onClick={() => setExpanded(true)}
          title={`展开显示剩余 ${hidden} 名成员`}
          data-testid="team-mem-expand"
        >
          +{hidden}
        </button>
      )}
      {expanded && members.length > maxVisible && (
        <button
          className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-300"
          onClick={() => setExpanded(false)}
          title="收起成员列表"
          data-testid="team-mem-collapse"
        >
          <ChevronRight className="h-3 w-3" />
        </button>
      )}
    </div>
  );
}
