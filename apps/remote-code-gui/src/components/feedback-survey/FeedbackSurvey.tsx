import React from 'react';
import { FeedbackSurveyView, isValidResponseInput } from './FeedbackSurveyView';
import { TranscriptSharePrompt } from './TranscriptSharePrompt';
import type { TranscriptShareResponse } from './TranscriptSharePrompt';
import type { FeedbackSurveyResponse, SurveyState } from './utils';

type Props = {
  state: SurveyState;
  lastResponse: FeedbackSurveyResponse | null;
  handleSelect: (selected: FeedbackSurveyResponse) => void;
  handleTranscriptSelect?: (selected: TranscriptShareResponse) => void;
  inputValue: string;
  setInputValue: (value: string) => void;
  onRequestFeedback?: () => void;
  message?: string;
};

export function FeedbackSurvey({
  state,
  lastResponse,
  handleSelect,
  handleTranscriptSelect,
  inputValue,
  setInputValue,
  onRequestFeedback,
  message,
}: Props): React.ReactElement | null {
  if (state === 'closed') {
    return null;
  }

  if (state === 'thanks') {
    return (
      <FeedbackSurveyThanks
        lastResponse={lastResponse}
        inputValue={inputValue}
        setInputValue={setInputValue}
        onRequestFeedback={onRequestFeedback}
      />
    );
  }

  if (state === 'submitted') {
    return (
      <div data-testid="feedback-submitted" className="mt-2">
        <span className="text-green-600 dark:text-green-400">
          ✓ Thanks for sharing your transcript!
        </span>
      </div>
    );
  }

  if (state === 'submitting') {
    return (
      <div data-testid="feedback-submitting" className="mt-2">
        <span className="text-gray-500 dark:text-gray-400">
          Sharing transcript…
        </span>
      </div>
    );
  }

  if (state === 'transcript_prompt') {
    if (!handleTranscriptSelect) {
      return null;
    }
    return (
      <TranscriptSharePrompt
        onSelect={handleTranscriptSelect}
        inputValue={inputValue}
        setInputValue={setInputValue}
      />
    );
  }

  // state === 'open'
  if (inputValue && !isValidResponseInput(inputValue)) {
    return null;
  }

  return (
    <FeedbackSurveyView
      onSelect={handleSelect}
      inputValue={inputValue}
      setInputValue={setInputValue}
      message={message}
    />
  );
}

type ThanksProps = {
  lastResponse: FeedbackSurveyResponse | null;
  inputValue: string;
  setInputValue: (value: string) => void;
  onRequestFeedback?: () => void;
};

function FeedbackSurveyThanks({
  lastResponse,
  onRequestFeedback,
}: ThanksProps): React.ReactElement {
  const showFollowUp = onRequestFeedback && lastResponse === 'good';
  const feedbackCommand = '/feedback';

  return (
    <div data-testid="feedback-thanks" className="mt-2 flex flex-col">
      <span className="text-green-600 dark:text-green-400">
        Thanks for the feedback!
      </span>
      {showFollowUp ? (
        <span className="text-gray-500 dark:text-gray-400">
          (Optional) Press{' '}
          <button
            data-testid="feedback-followup-btn"
            className="text-cyan-500 hover:text-cyan-600"
            onClick={onRequestFeedback}
          >
            [1]
          </button>{' '}
          to tell us what went well · {feedbackCommand}
        </span>
      ) : lastResponse === 'bad' ? (
        <span className="text-gray-500 dark:text-gray-400">
          Use /issue to report model behavior issues.
        </span>
      ) : (
        <span className="text-gray-500 dark:text-gray-400">
          Use {feedbackCommand} to share detailed feedback anytime.
        </span>
      )}
    </div>
  );
}
