import React from 'react';
import { cn } from '../../lib/utils';

export type TranscriptShareResponse = 'yes' | 'no' | 'dont_ask_again';

type Props = {
  onSelect: (option: TranscriptShareResponse) => void;
  inputValue: string;
  setInputValue: (value: string) => void;
};

const RESPONSE_OPTIONS: {
  key: string;
  label: string;
  value: TranscriptShareResponse;
}[] = [
  { key: '1', label: 'Yes', value: 'yes' },
  { key: '2', label: 'No', value: 'no' },
  { key: '3', label: "Don't ask again", value: 'dont_ask_again' },
];

export function TranscriptSharePrompt({
  onSelect,
}: Props): React.ReactElement {
  return (
    <div data-testid="transcript-share-prompt" className="mt-2 flex flex-col">
      <div className="flex items-start">
        <span className="mr-2 text-cyan-500">●</span>
        <span className="font-bold text-gray-900 dark:text-gray-100">
          Can Anthropic look at your session transcript to help us improve?
        </span>
      </div>
      <div className="ml-4 mt-1 text-sm text-gray-500 dark:text-gray-400">
        Learn more: https://docs.anthropic.com/en/docs/claude-code/data-usage
      </div>
      <div className="ml-4 mt-2 flex flex-wrap gap-4">
        {RESPONSE_OPTIONS.map((opt) => (
          <button
            key={opt.key}
            data-testid={`transcript-option-${opt.value}`}
            className={cn(
              'text-sm text-gray-700 dark:text-gray-300',
              'hover:text-cyan-600 dark:hover:text-cyan-400',
            )}
            onClick={() => onSelect(opt.value)}
          >
            <span className="text-cyan-500">{opt.key}</span>: {opt.label}
          </button>
        ))}
      </div>
    </div>
  );
}
