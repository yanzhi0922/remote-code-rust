import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { FeedbackThanks } from './FeedbackThanks';

describe('FeedbackThanks', () => {
  afterEach(() => { cleanup(); });

  it('renders thanks message', () => {
    const { getByTestId, getByText } = render(
      <FeedbackThanks rating="thumbs_up" onClose={() => {}} />,
    );
    expect(getByTestId('feedback-thanks')).toBeInTheDocument();
    expect(getByText(/感谢您的反馈/)).toBeInTheDocument();
  });

  it('shows comment when provided', () => {
    const { getByTestId, getByText } = render(
      <FeedbackThanks rating="thumbs_up" comment="Great job!" onClose={() => {}} />,
    );
    expect(getByTestId('feedback-comment-display')).toBeInTheDocument();
    expect(getByText('Great job!')).toBeInTheDocument();
  });

  it('does not show comment section when no comment', () => {
    const { queryByTestId } = render(
      <FeedbackThanks rating="thumbs_down" onClose={() => {}} />,
    );
    expect(queryByTestId('feedback-comment-display')).not.toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    const { getByTestId } = render(
      <FeedbackThanks rating="bug" onClose={onClose} />,
    );
    fireEvent.click(getByTestId('feedback-thanks-close'));
    expect(onClose).toHaveBeenCalled();
  });
});
