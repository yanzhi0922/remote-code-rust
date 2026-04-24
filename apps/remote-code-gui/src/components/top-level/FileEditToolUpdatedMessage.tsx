import { FilePen } from 'lucide-react';

export interface FileEditToolUpdatedMessageProps {
  filePath: string;
  description?: string;
}

export function FileEditToolUpdatedMessage({ filePath, description }: FileEditToolUpdatedMessageProps) {
  return (
    <div data-testid="file-edit-tool-updated" className="flex items-center gap-2 rounded bg-green-50 px-3 py-2">
      <FilePen className="h-4 w-4 shrink-0 text-green-600" />
      <div>
        <p className="text-sm font-medium text-green-700">文件已更新</p>
        <p className="text-xs text-green-600">{filePath}</p>
        {description && (
          <p className="mt-0.5 text-xs text-green-500">{description}</p>
        )}
      </div>
    </div>
  );
}
