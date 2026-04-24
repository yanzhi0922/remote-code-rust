import { cn } from '../../lib/utils';

export interface NotebookEditToolDiffProps {
  cellIndex: number;
  oldSource: string;
  newSource: string;
  className?: string;
}

export function NotebookEditToolDiff({ cellIndex, oldSource, newSource, className }: NotebookEditToolDiffProps) {
  return (
    <div className={cn('rounded-lg border border-slate-200 bg-white dark:border-slate-700', className)} data-testid="notebook-edit-tool-diff">
      <div className="border-b border-slate-200 px-3 py-1.5 text-xs font-medium text-slate-500 dark:border-slate-700">
        Cell #{cellIndex}
      </div>
      <div className="p-2 font-mono text-xs">
        <div className="bg-red-50 text-red-800 dark:bg-red-950/30 dark:text-red-400">
          <span className="mr-2 text-red-400">-</span>{oldSource}
        </div>
        <div className="bg-green-50 text-green-800 dark:bg-green-950/30 dark:text-green-400">
          <span className="mr-2 text-green-400">+</span>{newSource}
        </div>
      </div>
    </div>
  );
}
