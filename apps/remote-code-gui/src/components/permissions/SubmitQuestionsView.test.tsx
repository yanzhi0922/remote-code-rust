import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SubmitQuestionsView } from './SubmitQuestionsView';

describe('SubmitQuestionsView', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<SubmitQuestionsView answers={{}} onSubmit={vi.fn()} />);
    expect(screen.getByTestId('submit-questions-view')).toBeInTheDocument();
  });

  it('shows answer count', () => {
    render(<SubmitQuestionsView answers={{ q1: 'a', q2: 'b' }} onSubmit={vi.fn()} />);
    expect(screen.getByText('2')).toBeInTheDocument();
  });

  it('calls onSubmit', () => {
    const fn = vi.fn();
    render(<SubmitQuestionsView answers={{ q1: 'a' }} onSubmit={fn} />);
    fireEvent.click(screen.getByText('提交答案'));
    expect(fn).toHaveBeenCalled();
  });
});
