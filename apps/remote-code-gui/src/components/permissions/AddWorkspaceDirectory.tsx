import { FolderPlus } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface AddWorkspaceDirectoryProps {
  onAdd: (path: string) => void;
  className?: string;
}

export function AddWorkspaceDirectory({ onAdd, className }: AddWorkspaceDirectoryProps) {
  return (
    <button
      className={cn(
        'flex items-center gap-2 rounded-lg border border-dashed border-slate-300 px-3 py-2 text-sm text-slate-500 hover:border-blue-400 hover:text-blue-600 dark:border-slate-600 dark:hover:border-blue-500',
        className,
      )}
      data-testid="add-workspace-directory"
      onClick={() => onAdd('')}
    >
      <FolderPlus className="h-4 w-4" />
      <span>添加工作区目录</span>
    </button>
  );
}
