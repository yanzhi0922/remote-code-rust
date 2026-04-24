import { type ReactNode } from 'react';
import { Code, X, FileCode, GitCompare, CheckCircle } from 'lucide-react';
import { cn } from '../../lib/utils';

interface Props {
  onDone: () => void;
  ideName?: string;
  installedVersion?: string;
}

export function IdeOnboardingDialog({
  onDone,
  ideName = 'VS Code',
  installedVersion,
}: Props): ReactNode {
  const pluginOrExtension = ideName.toLowerCase().includes('jetbrains') ? 'plugin' : 'extension';

  return (
    <div
      data-testid="ide-onboarding-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-xl dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Code className="h-5 w-5 text-purple-500" />
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Welcome to Claude Code for {ideName}
            </h3>
          </div>
          <button
            data-testid="ide-onboarding-close"
            onClick={onDone}
            aria-label="Close"
            className="rounded p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {installedVersion && (
          <p className="mt-2 text-xs text-green-600 dark:text-green-400">
            installed {pluginOrExtension} v{installedVersion}
          </p>
        )}

        <div className="mt-4 space-y-2">
          <div className="flex items-start gap-2">
            <FileCode className="mt-0.5 h-4 w-4 text-blue-500" />
            <p className="text-sm text-gray-600 dark:text-gray-400">
              Claude has context of open files and selected lines
            </p>
          </div>
          <div className="flex items-start gap-2">
            <GitCompare className="mt-0.5 h-4 w-4 text-green-500" />
            <p className="text-sm text-gray-600 dark:text-gray-400">
              Apply diffs directly in the editor
            </p>
          </div>
          <div className="flex items-start gap-2">
            <CheckCircle className="mt-0.5 h-4 w-4 text-purple-500" />
            <p className="text-sm text-gray-600 dark:text-gray-400">
              Mention with Ctrl+Alt+K to invoke Claude
            </p>
          </div>
        </div>

        <div className="mt-4 flex justify-end">
          <button
            data-testid="ide-onboarding-done"
            onClick={onDone}
            className={cn(
              'rounded px-4 py-2 text-sm font-medium',
              'bg-purple-600 text-white hover:bg-purple-700',
            )}
          >
            Get Started
          </button>
        </div>
      </div>
    </div>
  );
}
