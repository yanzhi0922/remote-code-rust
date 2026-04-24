import { cn } from '../../lib/utils';

export interface FileWriteToolDiffProps {
  oldContent: string;
  newContent: string;
  filePath?: string;
  className?: string;
}

export function FileWriteToolDiff({
  oldContent,
  newContent,
  filePath,
  className,
}: FileWriteToolDiffProps) {
  const oldLines = oldContent.split('\n');
  const newLines = newContent.split('\n');

  return (
    <div
      className={cn('rounded-lg border border-slate-200 bg-white dark:border-slate-700', className)}
      data-testid="file-write-tool-diff"
    >
      {filePath && (
        <div className="border-b border-slate-200 px-3 py-1.5 text-xs font-medium text-slate-500 dark:border-slate-700">
          {filePath}
        </div>
      )}
      <div className="max-h-64 overflow-auto p-2 font-mono text-xs">
        {oldLines.map((line, i) => (
          <div key={`old-${i}`} className="bg-red-50 text-red-800 dark:bg-red-950/30 dark:text-red-400">
            <span className="mr-2 select-none text-red-400">-</span>
            {line}
          </div>
        ))}
        {newLines.map((line, i) => (
          <div key={`new-${i}`} className="bg-green-50 text-green-800 dark:bg-green-950/30 dark:text-green-400">
            <span className="mr-2 select-none text-green-400">+</span>
            {line}
          </div>
        ))}
      </div>
    </div>
  );
}
