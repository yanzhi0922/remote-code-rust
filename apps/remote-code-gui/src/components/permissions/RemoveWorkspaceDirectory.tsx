import { FolderMinus } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface RemoveWorkspaceDirectoryProps {
  path: string;
  onRemove: () => void;
  className?: string;
}

export function RemoveWorkspaceDirectory({ path, onRemove, className }: RemoveWorkspaceDirectoryProps) {
  return (
    <div className={cn('flex items-center gap-2 rounded-lg border border-slate-200 px-3 py-2 dark:border-slate-700', className)} data-testid="remove-workspace-directory">
      <FolderMinus className="h-4 w-4 text-slate-400" />
      <span className="min-w-0 flex-1 truncate font-mono text-sm text-slate-600">{path}</span>
      <button className="rounded px-2 py-1 text-xs text-red-600 hover:bg-red-50" onClick={onRemove}>移除</button>
    </div>
  );
}
