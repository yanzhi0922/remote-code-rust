import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PreviewQuestionView } from './PreviewQuestionView';

describe('PreviewQuestionView', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<PreviewQuestionView question="What?" />);
    expect(screen.getByTestId('preview-question-view')).toBeInTheDocument();
  });

  it('shows question', () => {
    render(<PreviewQuestionView question="Continue?" />);
    expect(screen.getByText('Continue?')).toBeInTheDocument();
  });

  it('shows answer when provided', () => {
    render(<PreviewQuestionView question="Q" answer="Yes" />);
    expect(screen.getByText('Yes')).toBeInTheDocument();
  });
});
