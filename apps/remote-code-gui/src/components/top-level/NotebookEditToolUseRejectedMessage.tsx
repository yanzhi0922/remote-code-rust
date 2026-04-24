import { FileX } from 'lucide-react';

export interface NotebookEditToolUseRejectedMessageProps {
  notebookPath: string;
  cellIndex?: number;
  reason?: string;
}

export function NotebookEditToolUseRejectedMessage({ notebookPath, cellIndex, reason }: NotebookEditToolUseRejectedMessageProps) {
  return (
    <div data-testid="notebook-edit-tool-rejected" className="flex items-center gap-2 rounded bg-amber-50 px-3 py-2">
      <FileX className="h-4 w-4 shrink-0 text-amber-600" />
      <div>
        <p className="text-sm font-medium text-amber-700">Notebook编辑被拒绝</p>
        <p className="text-xs text-amber-600">{notebookPath}{cellIndex !== undefined ? ` (单元格 ${cellIndex})` : ''}</p>
        {reason && (
          <p className="mt-0.5 text-xs text-amber-500">{reason}</p>
        )}
      </div>
    </div>
  );
}
