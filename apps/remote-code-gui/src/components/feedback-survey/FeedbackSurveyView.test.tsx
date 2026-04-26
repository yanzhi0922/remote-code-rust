import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { FeedbackSurveyView } from './FeedbackSurveyView';

describe('FeedbackSurveyView', () => {
  afterEach(() => { cleanup(); });

  it('renders survey view with rating options', () => {
    const { getByTestId } = render(
      <FeedbackSurveyView onSelect={() => {}} inputValue="" setInputValue={() => {}} />,
    );
    expect(getByTestId('feedback-survey-view')).toBeInTheDocument();
    expect(getByTestId('feedback-option-bad')).toBeInTheDocument();
    expect(getByTestId('feedback-option-fine')).toBeInTheDocument();
    expect(getByTestId('feedback-option-good')).toBeInTheDocument();
    expect(getByTestId('feedback-option-dismiss')).toBeInTheDocument();
  });

  it('calls onSelect with rating value when option clicked', () => {
    const onSelect = vi.fn();
    const { getByTestId } = render(
      <FeedbackSurveyView onSelect={onSelect} inputValue="" setInputValue={() => {}} />,
    );
    fireEvent.click(getByTestId('feedback-option-good'));
    expect(onSelect).toHaveBeenCalledWith('good');
  });

  it('calls onSelect with dismissed when dismiss clicked', () => {
    const onSelect = vi.fn();
    const { getByTestId } = render(
      <FeedbackSurveyView onSelect={onSelect} inputValue="" setInputValue={() => {}} />,
    );
    fireEvent.click(getByTestId('feedback-option-dismiss'));
    expect(onSelect).toHaveBeenCalledWith('dismissed');
  });

  it('shows custom message when provided', () => {
    const { getByText } = render(
      <FeedbackSurveyView
        onSelect={() => {}}
        inputValue=""
        setInputValue={() => {}}
        message="Custom survey message"
      />,
    );
    expect(getByText('Custom survey message')).toBeInTheDocument();
  });
});
