import React from 'react';
import { FileText } from 'lucide-react';

type Props = {
  filePath: string;
  children?: React.ReactNode;
  onClick?: (filePath: string) => void;
};

export function FilePathLink({
  filePath,
  children,
  onClick,
}: Props): React.ReactElement {
  const displayText = children ?? filePath;

  return (
    <button
      data-testid="file-path-link"
      className="inline-flex items-center gap-1 text-sm text-cyan-600 hover:text-cyan-700 hover:underline dark:text-cyan-400 dark:hover:text-cyan-300"
      onClick={() => onClick?.(filePath)}
      title={filePath}
    >
      <FileText className="h-3.5 w-3.5" />
      <span>{displayText}</span>
    </button>
  );
}
