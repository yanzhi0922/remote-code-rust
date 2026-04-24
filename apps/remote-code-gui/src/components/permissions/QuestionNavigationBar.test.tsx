import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { QuestionNavigationBar } from './QuestionNavigationBar';

describe('QuestionNavigationBar', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<QuestionNavigationBar current={0} total={5} onPrev={vi.fn()} onNext={vi.fn()} />);
    expect(screen.getByTestId('question-navigation-bar')).toBeInTheDocument();
  });

  it('shows current position', () => {
    render(<QuestionNavigationBar current={2} total={10} onPrev={vi.fn()} onNext={vi.fn()} />);
    expect(screen.getByText('3 / 10')).toBeInTheDocument();
  });

  it('calls onNext', () => {
    const onNext = vi.fn();
    render(<QuestionNavigationBar current={0} total={5} onPrev={vi.fn()} onNext={onNext} />);
    fireEvent.click(screen.getByTitle('下一个'));
    expect(onNext).toHaveBeenCalled();
  });

  it('calls onPrev', () => {
    const onPrev = vi.fn();
    render(<QuestionNavigationBar current={1} total={5} onPrev={onPrev} onNext={vi.fn()} />);
    fireEvent.click(screen.getByTitle('上一个'));
    expect(onPrev).toHaveBeenCalled();
  });
});
