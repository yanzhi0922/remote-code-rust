import { FileText } from 'lucide-react';
import type { StructuredDiffFile } from '../diff/StructuredDiff';

export interface StructuredDiffListProps {
  diffs: StructuredDiffFile[];
  selectedPath?: string;
  onSelect?: (filePath: string) => void;
}

export function StructuredDiffList({ diffs, selectedPath, onSelect }: StructuredDiffListProps) {
  if (diffs.length === 0) {
    return (
      <div data-testid="structured-diff-list-empty" className="py-4 text-center text-sm text-slate-400">
        无变更文件
      </div>
    );
  }

  return (
    <div data-testid="structured-diff-list" className="space-y-0.5">
      {diffs.map((diff) => {
        const additions = diff.hunks.reduce(
          (sum, h) => sum + h.changes.filter((c) => c.type === 'add').length, 0,
        );
        const deletions = diff.hunks.reduce(
          (sum, h) => sum + h.changes.filter((c) => c.type === 'delete').length, 0,
        );
        const isSelected = diff.file_path === selectedPath;

        return (
          <button
            key={diff.file_path}
            type="button"
            data-testid={`diff-list-item-${diff.file_path.replace(/[/\\]/g, '-')}`}
            className={`flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-slate-50 ${isSelected ? 'bg-blue-50' : ''}`}
            onClick={() => onSelect?.(diff.file_path)}
          >
            <FileText className="h-4 w-4 shrink-0 text-slate-400" />
            <span className="flex-1 truncate text-slate-700">{diff.file_path}</span>
            <span className="shrink-0 text-xs">
              <span className="text-green-600">+{additions}</span>
              {' '}
              <span className="text-red-600">-{deletions}</span>
            </span>
          </button>
        );
      })}
    </div>
  );
}
