import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { FeedbackSurvey } from './FeedbackSurvey';

afterEach(() => {
  cleanup();
});

const defaultProps = {
  state: 'closed' as const,
  lastResponse: null,
  handleSelect: vi.fn(),
  inputValue: '',
  setInputValue: vi.fn(),
};

describe('FeedbackSurvey', () => {
  it('renders nothing when state is closed', () => {
    const { container } = render(<FeedbackSurvey {...defaultProps} state="closed" />);
    expect(container.innerHTML).toBe('');
  });

  it('renders survey view when state is open', () => {
    render(<FeedbackSurvey {...defaultProps} state="open" />);
    expect(screen.getByTestId('feedback-survey-view')).toBeInTheDocument();
  });

  it('renders thanks message when state is thanks', () => {
    render(<FeedbackSurvey {...defaultProps} state="thanks" lastResponse="good" />);
    expect(screen.getByTestId('feedback-thanks')).toBeInTheDocument();
    expect(screen.getByText('Thanks for the feedback!')).toBeInTheDocument();
  });

  it('renders submitting state', () => {
    render(<FeedbackSurvey {...defaultProps} state="submitting" />);
    expect(screen.getByTestId('feedback-submitting')).toBeInTheDocument();
    expect(screen.getByText('Sharing transcript…')).toBeInTheDocument();
  });

  it('renders submitted state', () => {
    render(<FeedbackSurvey {...defaultProps} state="submitted" />);
    expect(screen.getByTestId('feedback-submitted')).toBeInTheDocument();
    expect(screen.getByText('✓ Thanks for sharing your transcript!')).toBeInTheDocument();
  });

  it('renders transcript prompt when state is transcript_prompt', () => {
    const handleTranscriptSelect = vi.fn();
    render(
      <FeedbackSurvey
        {...defaultProps}
        state="transcript_prompt"
        handleTranscriptSelect={handleTranscriptSelect}
      />,
    );
    expect(screen.getByTestId('transcript-share-prompt')).toBeInTheDocument();
  });

  it('shows follow-up button when lastResponse is good and onRequestFeedback is provided', () => {
    const onRequestFeedback = vi.fn();
    render(
      <FeedbackSurvey
        {...defaultProps}
        state="thanks"
        lastResponse="good"
        onRequestFeedback={onRequestFeedback}
      />,
    );
    expect(screen.getByTestId('feedback-followup-btn')).toBeInTheDocument();
  });

  it('shows issue hint when lastResponse is bad', () => {
    render(
      <FeedbackSurvey
        {...defaultProps}
        state="thanks"
        lastResponse="bad"
      />,
    );
    expect(screen.getByText(/Use \/issue to report model behavior issues/)).toBeInTheDocument();
  });

  it('does not render transcript prompt without handleTranscriptSelect', () => {
    const { container } = render(
      <FeedbackSurvey {...defaultProps} state="transcript_prompt" />,
    );
    expect(container.innerHTML).toBe('');
  });
});

describe('FeedbackSurveyView', () => {
  it('renders rating options', async () => {
    const { FeedbackSurveyView } = await import('./FeedbackSurveyView');
    const onSelect = vi.fn();
    render(
      <FeedbackSurveyView
        onSelect={onSelect}
        inputValue=""
        setInputValue={vi.fn()}
      />,
    );
    expect(screen.getByTestId('feedback-survey-view')).toBeInTheDocument();
    expect(screen.getByTestId('feedback-option-bad')).toBeInTheDocument();
    expect(screen.getByTestId('feedback-option-fine')).toBeInTheDocument();
    expect(screen.getByTestId('feedback-option-good')).toBeInTheDocument();
    expect(screen.getByTestId('feedback-option-dismiss')).toBeInTheDocument();
  });

  it('calls onSelect when clicking a rating option', async () => {
    const { FeedbackSurveyView } = await import('./FeedbackSurveyView');
    const onSelect = vi.fn();
    render(
      <FeedbackSurveyView
        onSelect={onSelect}
        inputValue=""
        setInputValue={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('feedback-option-good'));
    expect(onSelect).toHaveBeenCalledWith('good');
  });

  it('displays custom message', async () => {
    const { FeedbackSurveyView } = await import('./FeedbackSurveyView');
    render(
      <FeedbackSurveyView
        onSelect={vi.fn()}
        inputValue=""
        setInputValue={vi.fn()}
        message="Custom message"
      />,
    );
    expect(screen.getByText('Custom message')).toBeInTheDocument();
  });
});

describe('TranscriptSharePrompt', () => {
  it('renders transcript share options', async () => {
    const { TranscriptSharePrompt } = await import('./TranscriptSharePrompt');
    const onSelect = vi.fn();
    render(
      <TranscriptSharePrompt
        onSelect={onSelect}
        inputValue=""
        setInputValue={vi.fn()}
      />,
    );
    expect(screen.getByTestId('transcript-share-prompt')).toBeInTheDocument();
    expect(screen.getByTestId('transcript-option-yes')).toBeInTheDocument();
    expect(screen.getByTestId('transcript-option-no')).toBeInTheDocument();
    expect(screen.getByTestId('transcript-option-dont_ask_again')).toBeInTheDocument();
  });

  it('calls onSelect when clicking yes', async () => {
    const { TranscriptSharePrompt } = await import('./TranscriptSharePrompt');
    const onSelect = vi.fn();
    render(
      <TranscriptSharePrompt
        onSelect={onSelect}
        inputValue=""
        setInputValue={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('transcript-option-yes'));
    expect(onSelect).toHaveBeenCalledWith('yes');
  });

  it('calls onSelect when clicking dont_ask_again', async () => {
    const { TranscriptSharePrompt } = await import('./TranscriptSharePrompt');
    const onSelect = vi.fn();
    render(
      <TranscriptSharePrompt
        onSelect={onSelect}
        inputValue=""
        setInputValue={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('transcript-option-dont_ask_again'));
    expect(onSelect).toHaveBeenCalledWith('dont_ask_again');
  });
});

describe('useSurveyState', () => {
  it('initial state is closed', async () => {
    const { renderHook } = await import('@testing-library/react');
    const { useSurveyState } = await import('./useSurveyState');
    const { result } = renderHook(() =>
      useSurveyState({
        hideThanksAfterMs: 3000,
        onOpen: vi.fn(),
        onSelect: vi.fn(),
      }),
    );
    expect(result.current.state).toBe('closed');
    expect(result.current.lastResponse).toBeNull();
  });

  it('open changes state to open', async () => {
    const { renderHook, act } = await import('@testing-library/react');
    const { useSurveyState } = await import('./useSurveyState');
    const onOpen = vi.fn();
    const { result } = renderHook(() =>
      useSurveyState({
        hideThanksAfterMs: 3000,
        onOpen,
        onSelect: vi.fn(),
      }),
    );
    act(() => result.current.open());
    expect(result.current.state).toBe('open');
    expect(onOpen).toHaveBeenCalled();
  });

  it('handleSelect with dismissed closes survey', async () => {
    const { renderHook, act } = await import('@testing-library/react');
    const { useSurveyState } = await import('./useSurveyState');
    const onSelect = vi.fn();
    const { result } = renderHook(() =>
      useSurveyState({
        hideThanksAfterMs: 3000,
        onOpen: vi.fn(),
        onSelect,
      }),
    );
    act(() => result.current.open());
    act(() => result.current.handleSelect('dismissed'));
    expect(result.current.state).toBe('closed');
    expect(onSelect).toHaveBeenCalled();
  });
});

describe('utils', () => {
  it('isValidResponseInput validates correctly', async () => {
    const { isValidResponseInput } = await import('./FeedbackSurveyView');
    expect(isValidResponseInput('0')).toBe(true);
    expect(isValidResponseInput('1')).toBe(true);
    expect(isValidResponseInput('4')).toBe(false);
    expect(isValidResponseInput('a')).toBe(false);
  });

  it('getResponseFromInput returns correct response', async () => {
    const { getResponseFromInput } = await import('./utils');
    expect(getResponseFromInput('0')).toBe('dismissed');
    expect(getResponseFromInput('1')).toBe('bad');
    expect(getResponseFromInput('2')).toBe('fine');
    expect(getResponseFromInput('3')).toBe('good');
    expect(getResponseFromInput('a')).toBeNull();
  });
});
