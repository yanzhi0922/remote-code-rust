/**
 * Feedback survey utility types and helpers.
 */

export type FeedbackSurveyResponse = 'dismissed' | 'bad' | 'fine' | 'good';

export type TranscriptShareResponse = 'yes' | 'no' | 'dont_ask_again';

export type SurveyState =
  | 'closed'
  | 'open'
  | 'thanks'
  | 'transcript_prompt'
  | 'submitting'
  | 'submitted';

export interface FeedbackSurveyProps {
  state: SurveyState;
  lastResponse: FeedbackSurveyResponse | null;
  handleSelect: (selected: FeedbackSurveyResponse) => void;
  handleTranscriptSelect?: (selected: TranscriptShareResponse) => void;
  inputValue: string;
  setInputValue: (value: string) => void;
  onRequestFeedback?: () => void;
  message?: string;
}

const RESPONSE_INPUTS = ['0', '1', '2', '3'] as const;
type ResponseInput = (typeof RESPONSE_INPUTS)[number];

const inputToResponse: Record<ResponseInput, FeedbackSurveyResponse> = {
  '0': 'dismissed',
  '1': 'bad',
  '2': 'fine',
  '3': 'good',
} as const;

export const isValidResponseInput = (
  input: string,
): input is ResponseInput =>
  (RESPONSE_INPUTS as readonly string[]).includes(input);

export const RESPONSE_LABELS: Record<FeedbackSurveyResponse, string> = {
  dismissed: 'Dismiss',
  bad: 'Bad',
  fine: 'Fine',
  good: 'Good',
};

export const getResponseFromInput = (
  input: string,
): FeedbackSurveyResponse | null => {
  if (isValidResponseInput(input)) {
    return inputToResponse[input];
  }
  return null;
};
