import React from 'react';
import { cn } from '../../lib/utils';

type Props = {
  code: string;
  filePath: string;
  width?: number;
  dim?: boolean;
  language?: string;
};

export function HighlightedCode({
  code,
  filePath,
  dim = false,
  language,
}: Props): React.ReactElement {
  const lines = code.split('\n');

  return (
    <div
      data-testid="highlighted-code"
      className={cn(
        'overflow-x-auto rounded-md bg-gray-50 dark:bg-gray-900',
        dim && 'opacity-60',
      )}
    >
      <div className="flex items-center justify-between border-b border-gray-200 px-3 py-1 dark:border-gray-700">
        <span className="text-xs text-gray-500 dark:text-gray-400">{filePath}</span>
        {language && (
          <span className="rounded bg-gray-200 px-1.5 py-0.5 text-xs text-gray-600 dark:bg-gray-700 dark:text-gray-400">
            {language}
          </span>
        )}
      </div>
      <pre className="p-3">
        <code className="text-sm text-gray-800 dark:text-gray-200">
          {lines.map((line, i) => (
            <div key={i} className="flex">
              <span className="mr-4 inline-block w-8 select-none text-right text-xs text-gray-400 dark:text-gray-600">
                {i + 1}
              </span>
              <span className="flex-1">{line || '\u00A0'}</span>
            </div>
          ))}
        </code>
      </pre>
    </div>
  );
}
