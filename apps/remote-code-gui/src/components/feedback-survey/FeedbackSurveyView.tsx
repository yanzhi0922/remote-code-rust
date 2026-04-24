import React from 'react';
import { cn } from '../../lib/utils';
import type { FeedbackSurveyResponse } from './utils';
import { isValidResponseInput } from './utils';

type Props = {
  onSelect: (option: FeedbackSurveyResponse) => void;
  inputValue: string;
  setInputValue: (value: string) => void;
  message?: string;
};

const RESPONSE_INPUTS = ['0', '1', '2', '3'] as const;
type ResponseInput = (typeof RESPONSE_INPUTS)[number];

const inputToResponse: Record<ResponseInput, FeedbackSurveyResponse> = {
  '0': 'dismissed',
  '1': 'bad',
  '2': 'fine',
  '3': 'good',
} as const;

export { isValidResponseInput };

const DEFAULT_MESSAGE = 'How is the assistant doing this session? (optional)';

const RATING_OPTIONS: {
  key: string;
  label: string;
  value: FeedbackSurveyResponse;
}[] = [
  { key: '1', label: 'Bad', value: 'bad' },
  { key: '2', label: 'Fine', value: 'fine' },
  { key: '3', label: 'Good', value: 'good' },
];

export function FeedbackSurveyView({
  onSelect,
  message = DEFAULT_MESSAGE,
}: Props): React.ReactElement {
  return (
    <div data-testid="feedback-survey-view" className="mt-2 flex flex-col">
      <div className="flex items-center">
        <span className="mr-2 text-cyan-500">●</span>
        <span className="font-bold text-gray-900 dark:text-gray-100">
          {message}
        </span>
      </div>
      <div className="ml-4 mt-2 flex flex-wrap gap-4">
        {RATING_OPTIONS.map((opt) => (
          <button
            key={opt.key}
            data-testid={`feedback-option-${opt.value}`}
            className={cn(
              'text-sm text-gray-700 dark:text-gray-300',
              'hover:text-cyan-600 dark:hover:text-cyan-400',
            )}
            onClick={() => onSelect(opt.value)}
          >
            <span className="text-cyan-500">{opt.key}</span>: {opt.label}
          </button>
        ))}
        <button
          data-testid="feedback-option-dismiss"
          className={cn(
            'text-sm text-gray-700 dark:text-gray-300',
            'hover:text-cyan-600 dark:hover:text-cyan-400',
          )}
          onClick={() => onSelect('dismissed')}
        >
          <span className="text-cyan-500">0</span>: Dismiss
        </button>
      </div>
    </div>
  );
}
