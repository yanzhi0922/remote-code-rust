import { type ReactNode } from 'react';
import { AlertTriangle } from 'lucide-react';
import { cn } from '../../lib/utils';

type MemoryStatus = 'normal' | 'high' | 'critical';

interface Props {
  heapUsed: number;
  status: MemoryStatus;
}

function formatFileSize(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`;
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(1)} MB`;
  if (bytes >= 1_024) return `${(bytes / 1_024).toFixed(1)} KB`;
  return `${bytes} B`;
}

export function MemoryUsageIndicator({ heapUsed, status }: Props): ReactNode {
  if (status === 'normal') return null;

  const color = status === 'critical' ? 'text-red-500' : 'text-yellow-500';

  return (
    <div
      data-testid="memory-usage-indicator"
      className={cn('flex items-center gap-1.5 text-xs', color)}
    >
      <AlertTriangle className="h-3.5 w-3.5" />
      <span>High memory usage ({formatFileSize(heapUsed)})</span>
    </div>
  );
}
