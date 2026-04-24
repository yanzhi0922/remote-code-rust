import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FeedbackDialog } from './FeedbackDialog';
import { FeedbackThanks } from './FeedbackThanks';

afterEach(() => {
  cleanup();
});

// ─── FeedbackDialog ─────────────────────────────────────────────────
describe('FeedbackDialog', () => {
  it('renders nothing when visible is false', () => {
    render(<FeedbackDialog visible={false} onSubmit={vi.fn()} onClose={vi.fn()} />);
    expect(screen.queryByTestId('feedback-dialog')).not.toBeInTheDocument();
  });

  it('renders dialog when visible is true', () => {
    render(<FeedbackDialog visible={true} onSubmit={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTestId('feedback-dialog')).toBeInTheDocument();
    expect(screen.getByText('发送反馈')).toBeInTheDocument();
  });

  it('shows three rating buttons', () => {
    render(<FeedbackDialog visible={true} onSubmit={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTestId('rating-thumbs_up')).toBeInTheDocument();
    expect(screen.getByTestId('rating-thumbs_down')).toBeInTheDocument();
    expect(screen.getByTestId('rating-bug')).toBeInTheDocument();
  });

  it('highlights selected rating', () => {
    render(<FeedbackDialog visible={true} onSubmit={vi.fn()} onClose={vi.fn()} />);
    const btn = screen.getByTestId('rating-thumbs_up');
    fireEvent.click(btn);
    expect(btn.className).toContain('border-blue-500');
  });

  it('calls onSubmit with rating and comment', () => {
    const onSubmit = vi.fn();
    render(<FeedbackDialog visible={true} onSubmit={onSubmit} onClose={vi.fn()} />);

    fireEvent.click(screen.getByTestId('rating-thumbs_up'));
    fireEvent.change(screen.getByTestId('feedback-comment'), { target: { value: 'Great!' } });
    fireEvent.click(screen.getByTestId('feedback-submit'));

    expect(onSubmit).toHaveBeenCalledWith('thumbs_up', 'Great!');
  });

  it('shows thank you message after submit', () => {
    render(<FeedbackDialog visible={true} onSubmit={vi.fn()} onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId('rating-bug'));
    fireEvent.click(screen.getByTestId('feedback-submit'));
    expect(screen.getByTestId('feedback-thanks')).toBeInTheDocument();
    expect(screen.getByText('感谢您的反馈！')).toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    render(<FeedbackDialog visible={true} onSubmit={vi.fn()} onClose={onClose} />);
    fireEvent.click(screen.getByTestId('feedback-close'));
    expect(onClose).toHaveBeenCalled();
  });

  it('disables submit when no rating selected', () => {
    render(<FeedbackDialog visible={true} onSubmit={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTestId('feedback-submit')).toBeDisabled();
  });
});

// ─── FeedbackThanks ─────────────────────────────────────────────────
describe('FeedbackThanks', () => {
  it('renders thank you title', () => {
    render(<FeedbackThanks rating="thumbs_up" onClose={vi.fn()} />);
    expect(screen.getByText('感谢您的反馈！')).toBeInTheDocument();
  });

  it('displays thumbs_up rating', () => {
    render(<FeedbackThanks rating="thumbs_up" onClose={vi.fn()} />);
    expect(screen.getByTestId('feedback-rating-display')).toHaveTextContent('👍 赞');
  });

  it('displays bug rating', () => {
    render(<FeedbackThanks rating="bug" onClose={vi.fn()} />);
    expect(screen.getByTestId('feedback-rating-display')).toHaveTextContent('🐛 Bug');
  });

  it('displays comment when provided', () => {
    render(<FeedbackThanks rating="thumbs_down" comment="Not helpful" onClose={vi.fn()} />);
    expect(screen.getByTestId('feedback-comment-display')).toHaveTextContent('Not helpful');
  });

  it('hides comment section when not provided', () => {
    render(<FeedbackThanks rating="thumbs_up" onClose={vi.fn()} />);
    expect(screen.queryByTestId('feedback-comment-display')).not.toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    render(<FeedbackThanks rating="thumbs_up" onClose={onClose} />);
    fireEvent.click(screen.getByTestId('feedback-thanks-close'));
    expect(onClose).toHaveBeenCalled();
  });
});
