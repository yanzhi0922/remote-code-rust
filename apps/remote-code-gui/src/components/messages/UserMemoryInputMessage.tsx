import { Brain } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface UserMemoryInputMessageProps {
  content: string;
  memoryKey?: string;
  operation?: 'save' | 'update' | 'delete';
  className?: string;
}

export function UserMemoryInputMessage({
  content,
  memoryKey,
  operation,
  className,
}: UserMemoryInputMessageProps) {
  const opLabel: Record<string, string> = {
    save: '保存',
    update: '更新',
    delete: '删除',
  };

  return (
    <div
      className={cn(
        'rounded-lg border border-violet-200 bg-violet-50 px-4 py-3 dark:border-violet-800 dark:bg-violet-950/30',
        className,
      )}
      data-testid="user-memory-input-message"
    >
      <div className="flex items-center gap-2 text-xs text-violet-600 dark:text-violet-400">
        <Brain className="h-3.5 w-3.5" />
        <span className="font-medium">记忆</span>
        {operation && <span>· {opLabel[operation] ?? operation}</span>}
        {memoryKey && (
          <span className="ml-1 truncate font-mono text-violet-500">{memoryKey}</span>
        )}
      </div>
      <p className="mt-1 text-sm whitespace-pre-wrap text-violet-900 dark:text-violet-200">
        {content}
      </p>
    </div>
  );
}
