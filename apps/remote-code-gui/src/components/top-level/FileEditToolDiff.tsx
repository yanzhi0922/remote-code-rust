import { type ReactNode, useState, useMemo } from 'react';
import { FileText } from 'lucide-react';
import { cn } from '../../lib/utils';

interface DiffLine {
  type: 'add' | 'remove' | 'context';
  content: string;
  lineNumber?: number;
}

interface Props {
  filePath: string;
  diffLines: DiffLine[];
}

export function FileEditToolDiff({ filePath, diffLines }: Props): ReactNode {
  const [expanded, setExpanded] = useState(false);

  const displayLines = useMemo(() => {
    if (expanded || diffLines.length <= 20) return diffLines;
    return diffLines.slice(0, 20);
  }, [diffLines, expanded]);

  const hasMore = !expanded && diffLines.length > 20;

  return (
    <div data-testid="file-edit-diff" className="flex flex-col rounded border border-gray-200 dark:border-gray-700">
      <div className="flex items-center gap-2 border-b border-gray-200 px-3 py-1.5 dark:border-gray-700">
        <FileText className="h-3.5 w-3.5 text-gray-400" />
        <span className="text-xs font-medium text-gray-600 dark:text-gray-400">{filePath}</span>
      </div>
      <div className="flex flex-col overflow-x-auto">
        {displayLines.map((line, i) => (
          <div
            key={i}
            data-testid={`diff-line-${i}`}
            className={cn(
              'flex font-mono text-xs leading-5',
              line.type === 'add' && 'bg-green-50 text-green-800 dark:bg-green-900/20 dark:text-green-300',
              line.type === 'remove' && 'bg-red-50 text-red-800 dark:bg-red-900/20 dark:text-red-300',
              line.type === 'context' && 'text-gray-600 dark:text-gray-400',
            )}
          >
            {line.lineNumber != null && (
              <span className="w-10 shrink-0 select-none text-right text-gray-400">{line.lineNumber}</span>
            )}
            <span className="shrink-0 w-4 text-center">
              {line.type === 'add' ? '+' : line.type === 'remove' ? '-' : ' '}
            </span>
            <span className="whitespace-pre">{line.content}</span>
          </div>
        ))}
        {hasMore && (
          <button
            data-testid="diff-expand"
            onClick={() => setExpanded(true)}
            className="px-3 py-1 text-xs text-blue-500 hover:text-blue-700"
          >
            Show all {diffLines.length} lines
          </button>
        )}
      </div>
    </div>
  );
}
