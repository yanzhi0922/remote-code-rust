import { FileX } from 'lucide-react';

export interface FileEditToolUseRejectedMessageProps {
  filePath: string;
  reason?: string;
}

export function FileEditToolUseRejectedMessage({ filePath, reason }: FileEditToolUseRejectedMessageProps) {
  return (
    <div data-testid="file-edit-tool-rejected" className="flex items-center gap-2 rounded bg-amber-50 px-3 py-2">
      <FileX className="h-4 w-4 shrink-0 text-amber-600" />
      <div>
        <p className="text-sm font-medium text-amber-700">文件编辑被拒绝</p>
        <p className="text-xs text-amber-600">{filePath}</p>
        {reason && (
          <p className="mt-0.5 text-xs text-amber-500">{reason}</p>
        )}
      </div>
    </div>
  );
}
