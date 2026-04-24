import { FileText, Image as ImageIcon, File } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface AttachmentMessageProps {
  fileName: string;
  fileType?: string;
  fileSize?: number;
  preview?: string;
  className?: string;
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function getFileIcon(fileType?: string) {
  if (!fileType) return File;
  if (fileType.startsWith('image/')) return ImageIcon;
  return FileText;
}

export function AttachmentMessage({
  fileName,
  fileType,
  fileSize,
  preview,
  className,
}: AttachmentMessageProps) {
  const Icon = getFileIcon(fileType);

  return (
    <div
      className={cn(
        'flex items-center gap-3 rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 dark:border-slate-700 dark:bg-slate-800/50',
        className,
      )}
      data-testid="attachment-message"
    >
      {preview && fileType?.startsWith('image/') ? (
        <img
          src={preview}
          alt={fileName}
          className="h-10 w-10 rounded object-cover"
          data-testid="attachment-preview"
        />
      ) : (
        <Icon className="h-5 w-5 shrink-0 text-slate-400" />
      )}
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium text-slate-700 dark:text-slate-300">
          {fileName}
        </p>
        <div className="flex items-center gap-2 text-xs text-slate-400">
          {fileType && <span>{fileType}</span>}
          {fileSize != null && <span>{formatFileSize(fileSize)}</span>}
        </div>
      </div>
    </div>
  );
}
