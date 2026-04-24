import { Terminal } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface UserLocalCommandOutputMessageProps {
  command?: string;
  output: string;
  exitCode?: number;
  className?: string;
}

export function UserLocalCommandOutputMessage({
  command,
  output,
  exitCode,
  className,
}: UserLocalCommandOutputMessageProps) {
  return (
    <div
      className={cn(
        'rounded-lg border border-slate-200 bg-slate-50 dark:border-slate-700 dark:bg-slate-800/50',
        className,
      )}
      data-testid="user-local-command-output-message"
    >
      <div className="flex items-center gap-2 border-b border-slate-200 px-3 py-1.5 dark:border-slate-700">
        <Terminal className="h-3.5 w-3.5 text-slate-400" />
        {command && (
          <span className="font-mono text-xs text-slate-600 dark:text-slate-400">
            {command}
          </span>
        )}
        {exitCode != null && (
          <span
            className={cn(
              'ml-auto rounded px-1.5 py-0.5 text-xs font-medium',
              exitCode === 0
                ? 'bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-400'
                : 'bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-400',
            )}
          >
            exit {exitCode}
          </span>
        )}
      </div>
      <pre className="max-h-48 overflow-auto p-3 text-xs text-slate-700 dark:text-slate-300">
        {output}
      </pre>
    </div>
  );
}
