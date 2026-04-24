import { type ReactNode } from 'react';
import { cn } from '../../lib/utils';

interface Props {
  imageId: number;
  isSelected?: boolean;
  onClick?: () => void;
}

export function ClickableImageRef({ imageId, isSelected = false, onClick }: Props): ReactNode {
  const displayText = `[Image #${imageId}]`;

  return (
    <button
      data-testid={`clickable-image-ref-${imageId}`}
      onClick={onClick}
      className={cn(
        'inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium transition-colors',
        isSelected
          ? 'bg-blue-500 text-white'
          : 'bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600',
      )}
    >
      {displayText}
    </button>
  );
}
