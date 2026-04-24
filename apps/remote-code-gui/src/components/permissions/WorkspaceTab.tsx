import { FolderOpen } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface WorkspaceTabProps {
  directories: string[];
  onRemove: (path: string) => void;
  className?: string;
}

export function WorkspaceTab({ directories, onRemove, className }: WorkspaceTabProps) {
  return (
    <div className={cn('space-y-2', className)} data-testid="workspace-tab">
      {directories.length === 0 && (
        <p className="py-4 text-center text-sm text-slate-400">暂无工作区目录</p>
      )}
      {directories.map((dir) => (
        <div key={dir} className="flex items-center gap-2 rounded-lg border border-slate-200 px-3 py-2 dark:border-slate-700">
          <FolderOpen className="h-4 w-4 text-slate-400" />
          <span className="min-w-0 flex-1 truncate font-mono text-sm text-slate-600">{dir}</span>
          <button className="text-xs text-red-500 hover:text-red-700" onClick={() => onRemove(dir)}>移除</button>
        </div>
      ))}
    </div>
  );
}
