import React from 'react';
import { FileText, Plus } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface MemoryFileInfo {
  path: string;
  type: 'User' | 'Project' | 'Nested';
  exists: boolean;
  description?: string;
}

type Props = {
  files: MemoryFileInfo[];
  onSelect: (path: string) => void;
  onCancel: () => void;
};

export function MemoryFileSelector({
  files,
  onSelect,
  onCancel,
}: Props): React.ReactElement {
  return (
    <div
      data-testid="memory-file-selector"
      className="rounded-lg border border-gray-200 bg-white p-4 shadow-lg dark:border-gray-700 dark:bg-gray-800"
    >
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          Memory Files
        </h3>
        <button
          data-testid="memory-selector-cancel"
          onClick={onCancel}
          className="text-sm text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
        >
          Cancel
        </button>
      </div>
      <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
        Select a memory file to edit
      </p>
      <div className="mt-3 flex flex-col gap-1">
        {files.length === 0 ? (
          <p className="text-sm text-gray-500 dark:text-gray-400">
            No memory files found. Create one in .claude/CLAUDE.md or ~/.claude/CLAUDE.md
          </p>
        ) : (
          files.map((file) => (
            <button
              key={file.path}
              data-testid={`memory-file-${file.path.replace(/[^a-zA-Z0-9]/g, '-')}`}
              className={cn(
                'flex items-center justify-between rounded-md px-3 py-2 text-left transition-colors',
                'hover:bg-gray-50 dark:hover:bg-gray-700/50',
                !file.exists && 'opacity-60',
              )}
              onClick={() => onSelect(file.path)}
            >
              <div className="flex items-center gap-2">
                <FileText className="h-4 w-4 text-gray-400" />
                <div className="flex flex-col">
                  <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
                    {file.type === 'User'
                      ? 'User memory'
                      : file.type === 'Project'
                        ? 'Project memory'
                        : file.path}
                    {!file.exists && (
                      <span className="ml-1 text-xs text-gray-400">(new)</span>
                    )}
                  </span>
                  {file.description && (
                    <span className="text-xs text-gray-500 dark:text-gray-400">
                      {file.description}
                    </span>
                  )}
                </div>
              </div>
              {!file.exists && (
                <Plus className="h-4 w-4 text-gray-400" />
              )}
            </button>
          ))
        )}
      </div>
    </div>
  );
}
