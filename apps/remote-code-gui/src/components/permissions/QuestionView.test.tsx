import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { QuestionView } from './QuestionView';

describe('QuestionView', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<QuestionView question="Q" index={0} total={3} />);
    expect(screen.getByTestId('question-view')).toBeInTheDocument();
  });

  it('shows question text', () => {
    render(<QuestionView question="What is this?" index={0} total={1} />);
    expect(screen.getByText('What is this?')).toBeInTheDocument();
  });

  it('shows position', () => {
    render(<QuestionView question="Q" index={2} total={5} />);
    expect(screen.getByText('问题 3/5')).toBeInTheDocument();
  });
});
