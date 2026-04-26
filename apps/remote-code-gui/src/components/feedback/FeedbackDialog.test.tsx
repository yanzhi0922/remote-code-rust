import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { FeedbackDialog } from './FeedbackDialog';

describe('FeedbackDialog', () => {
  afterEach(() => { cleanup(); });

  it('returns null when not visible', () => {
    const { container } = render(
      <FeedbackDialog visible={false} onSubmit={() => {}} onClose={() => {}} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('renders dialog when visible', () => {
    const { getByTestId, getByText } = render(
      <FeedbackDialog visible onSubmit={() => {}} onClose={() => {}} />,
    );
    expect(getByTestId('feedback-dialog')).toBeInTheDocument();
    expect(getByText('发送反馈')).toBeInTheDocument();
  });

  it('renders rating options', () => {
    const { getByText } = render(
      <FeedbackDialog visible onSubmit={() => {}} onClose={() => {}} />,
    );
    expect(getByText(/赞/)).toBeInTheDocument();
    expect(getByText(/踩/)).toBeInTheDocument();
    expect(getByText(/Bug/)).toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    const { getByTestId } = render(
      <FeedbackDialog visible onSubmit={() => {}} onClose={onClose} />,
    );
    fireEvent.click(getByTestId('feedback-close'));
    expect(onClose).toHaveBeenCalled();
  });
});
