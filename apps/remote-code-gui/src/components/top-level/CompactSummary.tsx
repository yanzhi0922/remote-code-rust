import { type ReactNode } from 'react';

interface SummarizeMetadata {
  messagesSummarized: number;
  direction: 'up_to' | 'from_here';
  userContext?: string;
}

interface Props {
  textContent?: string;
  metadata?: SummarizeMetadata | null;
  isTranscriptMode?: boolean;
  onExpand?: () => void;
}

export function CompactSummary({
  textContent = '',
  metadata,
  isTranscriptMode = false,
  onExpand,
}: Props): ReactNode {
  if (metadata) {
    return (
      <div data-testid="compact-summary" className="mt-2 flex flex-col">
        <div className="flex items-start gap-2">
          <span className="mt-0.5 text-sm text-gray-500">●</span>
          <div className="flex flex-col">
            <span className="text-sm font-semibold text-gray-800 dark:text-gray-200">
              Summarized conversation
            </span>
            {!isTranscriptMode && (
              <div className="mt-0.5 flex flex-col text-xs text-gray-500 dark:text-gray-400">
                <span>
                  Summarized {metadata.messagesSummarized} messages{' '}
                  {metadata.direction === 'up_to' ? 'up to this point' : 'from this point'}
                </span>
                {metadata.userContext && (
                  <span>Context: &ldquo;{metadata.userContext}&rdquo;</span>
                )}
                {onExpand && (
                  <button
                    data-testid="compact-expand"
                    onClick={onExpand}
                    className="mt-0.5 text-blue-500 hover:text-blue-700"
                  >
                    (ctrl+o to expand history)
                  </button>
                )}
              </div>
            )}
            {isTranscriptMode && textContent && (
              <p className="mt-1 text-sm text-gray-700 dark:text-gray-300">{textContent}</p>
            )}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div data-testid="compact-summary" className="mt-2 flex flex-col">
      <div className="flex items-start gap-2">
        <span className="mt-0.5 text-sm text-gray-500">●</span>
        <div className="flex flex-col">
          <div className="flex items-center gap-1">
            <span className="text-sm font-semibold text-gray-800 dark:text-gray-200">
              Compact summary
            </span>
            {!isTranscriptMode && onExpand && (
              <span className="text-xs text-gray-500">
                {' '}
                <button
                  data-testid="compact-expand"
                  onClick={onExpand}
                  className="text-blue-500 hover:text-blue-700"
                >
                  (ctrl+o to expand)
                </button>
              </span>
            )}
          </div>
          {isTranscriptMode && textContent && (
            <p className="mt-1 text-sm text-gray-700 dark:text-gray-300">{textContent}</p>
          )}
        </div>
      </div>
    </div>
  );
}
