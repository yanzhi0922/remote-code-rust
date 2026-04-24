import { type ReactNode } from 'react';
import { FileText, X, ExternalLink } from 'lucide-react';
import { cn } from '../../lib/utils';

interface ExternalClaudeMdInclude {
  path: string;
  source: string;
}

interface Props {
  onDone: () => void;
  isStandaloneDialog?: boolean;
  externalIncludes?: ExternalClaudeMdInclude[];
}

export function ClaudeMdExternalIncludesDialog({
  onDone,
  isStandaloneDialog = false,
  externalIncludes,
}: Props): ReactNode {
  const handleAccept = () => onDone();
  const handleReject = () => onDone();

  return (
    <div
      data-testid="claude-md-external-includes-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <FileText className="h-5 w-5 text-purple-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              {isStandaloneDialog ? 'CLAUDE.md External Includes' : 'External Imports Detected'}
            </h3>
          </div>
          <button
            data-testid="claude-md-external-close"
            onClick={handleReject}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-3 text-sm text-gray-600 dark:text-gray-400">
          This project's CLAUDE.md imports files outside the current working directory.
          Never allow this for third-party repositories.
        </p>

        {externalIncludes && externalIncludes.length > 0 && (
          <div className="mt-3">
            <p className="text-xs font-medium text-gray-500 dark:text-gray-500">External imports:</p>
            <ul className="mt-1 space-y-1">
              {externalIncludes.map((inc, index) => (
                <li key={index} className="text-xs text-gray-600 dark:text-gray-400">
                  {inc.path} ({inc.source})
                </li>
              ))}
            </ul>
          </div>
        )}

        <p className="mt-3 text-xs text-gray-500 dark:text-gray-500">
          Important: Only use Claude Code with files you trust.{' '}
          <a
            href="https://docs.anthropic.com/en/docs/claude-code/security"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-0.5 text-blue-600 dark:text-blue-400"
          >
            Learn more <ExternalLink className="h-3 w-3" />
          </a>
        </p>

        <div className="mt-4 flex justify-end gap-2">
          <button
            data-testid="claude-md-external-no"
            onClick={handleReject}
            className={cn(
              'rounded px-4 py-2 text-sm font-medium',
              'bg-gray-100 text-gray-700 hover:bg-gray-200',
              'dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600',
            )}
          >
            No, disable external imports
          </button>
          <button
            data-testid="claude-md-external-yes"
            onClick={handleAccept}
            className="rounded bg-purple-600 px-4 py-2 text-sm font-medium text-white hover:bg-purple-700"
          >
            Yes, allow external imports
          </button>
        </div>
      </div>
    </div>
  );
}
