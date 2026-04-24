import { type ReactNode, useState } from 'react';
import { Folder, FolderInput } from 'lucide-react';
import { WizardDialogLayout, useWizard } from '../../wizard';
import type { ScheduledTaskWizardData } from '../types';

function isSafePath(path: string): boolean {
  const normalized = path.trim();
  if (!normalized) return false;
  if (normalized.startsWith('/')) return true;
  if (normalized.startsWith('~')) return true;
  if (normalized.includes('..')) return false;
  return true;
}

export function FolderStep(): ReactNode {
  const { goNext, goBack, wizardData, setWizardData } =
    useWizard<ScheduledTaskWizardData>();
  const [customPath, setCustomPath] = useState(false);
  const [pathValue, setPathValue] = useState(wizardData.folder ?? '');
  const [pathError, setPathError] = useState<string | null>(null);

  const handleFolderSelect = (value: string) => {
    if (value === '__custom__') {
      setCustomPath(true);
      return;
    }
    setWizardData(prev => ({ ...prev, folder: value }));
    goNext();
  };

  const handleCustomSubmit = () => {
    const trimmed = pathValue.trim();
    if (!trimmed) {
      setPathError('Path cannot be empty');
      return;
    }
    if (!isSafePath(trimmed)) {
      setPathError('Invalid path');
      return;
    }
    setPathError(null);
    setWizardData(prev => ({ ...prev, folder: trimmed }));
    goNext();
  };

  if (customPath) {
    return (
      <WizardDialogLayout subtitle="Working directory">
        <div className="flex flex-col gap-2">
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Enter the full path to the working directory:
          </p>
          <div className="flex items-center gap-2">
            <FolderInput className="h-4 w-4 text-gray-400" />
            <input
              data-testid="custom-folder-input"
              type="text"
              className="flex-1 rounded border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
              value={pathValue}
              onChange={(e) => { setPathValue(e.target.value); setPathError(null); }}
              onKeyDown={(e) => { if (e.key === 'Enter') handleCustomSubmit(); }}
              placeholder="/path/to/project"
            />
          </div>
          {pathError && (
            <p data-testid="folder-error" className="text-sm text-red-500">{pathError}</p>
          )}
          <div className="flex gap-2 mt-2">
            <button
              data-testid="folder-custom-back"
              onClick={() => setCustomPath(false)}
              className="rounded border border-gray-300 px-3 py-1.5 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
            >
              Back
            </button>
            <button
              data-testid="folder-custom-submit"
              onClick={handleCustomSubmit}
              className="rounded bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700"
            >
              Next
            </button>
          </div>
        </div>
      </WizardDialogLayout>
    );
  }

  return (
    <WizardDialogLayout subtitle="Working directory">
      <div className="flex flex-col gap-2">
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Select the folder where this task will run.
        </p>
        <div className="flex flex-col gap-1" data-testid="folder-options">
          <button
            data-testid="folder-current"
            onClick={() => handleFolderSelect('/current/project')}
            className="flex items-center gap-2 rounded px-3 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700"
          >
            <Folder className="h-4 w-4 text-gray-400" />
            <span>Current project</span>
            <span className="ml-auto text-xs text-gray-400">/current/project</span>
          </button>
          <button
            data-testid="folder-custom"
            onClick={() => handleFolderSelect('__custom__')}
            className="flex items-center gap-2 rounded px-3 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700"
          >
            <FolderInput className="h-4 w-4 text-gray-400" />
            <span>Choose a different folder</span>
          </button>
        </div>
        <button
          data-testid="folder-back"
          onClick={goBack}
          className="mt-2 rounded border border-gray-300 px-3 py-1.5 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
        >
          Back
        </button>
      </div>
    </WizardDialogLayout>
  );
}
