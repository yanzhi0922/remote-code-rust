import { Ban } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface DenialRecord {
  toolName: string;
  reason: string;
  timestamp: string;
}

export interface RecentDenialsTabProps {
  denials: DenialRecord[];
  className?: string;
}

export function RecentDenialsTab({ denials, className }: RecentDenialsTabProps) {
  return (
    <div className={cn('space-y-2', className)} data-testid="recent-denials-tab">
      {denials.length === 0 && (
        <p className="py-4 text-center text-sm text-slate-400">暂无拒绝记录</p>
      )}
      {denials.map((d, i) => (
        <div key={i} className="flex items-start gap-2 rounded-lg border border-red-100 bg-red-50 p-2 dark:border-red-900 dark:bg-red-950/30">
          <Ban className="mt-0.5 h-4 w-4 shrink-0 text-red-400" />
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium text-red-700">{d.toolName}</p>
            <p className="text-xs text-red-500">{d.reason}</p>
            <p className="text-xs text-red-300">{d.timestamp}</p>
          </div>
        </div>
      ))}
    </div>
  );
}
