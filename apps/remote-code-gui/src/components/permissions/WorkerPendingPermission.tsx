import { Clock } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface WorkerPendingPermissionProps {
  workerName: string;
  toolName: string;
  className?: string;
}

export function WorkerPendingPermission({ workerName, toolName, className }: WorkerPendingPermissionProps) {
  return (
    <div className={cn('flex items-center gap-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 dark:border-amber-800 dark:bg-amber-950/30', className)} data-testid="worker-pending-permission">
      <Clock className="h-4 w-4 animate-pulse text-amber-500" />
      <span className="text-sm text-amber-800">
        <strong>{workerName}</strong> 等待权限: {toolName}
      </span>
    </div>
  );
}
