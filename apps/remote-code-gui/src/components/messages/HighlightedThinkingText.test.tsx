import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { HighlightedThinkingText } from './HighlightedThinkingText';

describe('HighlightedThinkingText', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<HighlightedThinkingText text="thinking..." />);
    expect(screen.getByTestId('highlighted-thinking-text')).toBeInTheDocument();
  });

  it('displays text content', () => {
    render(<HighlightedThinkingText text="deep thought" />);
    expect(screen.getByText('deep thought')).toBeInTheDocument();
  });

  it('highlights matching terms', () => {
    render(
      <HighlightedThinkingText text="consider using React hooks" highlights={['React']} />,
    );
    expect(screen.getByText('React')).toBeInTheDocument();
    expect(screen.getByText('React').tagName).toBe('MARK');
  });

  it('renders plain text without highlights', () => {
    render(<HighlightedThinkingText text="plain text" />);
    expect(screen.getByText('plain text')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(
      <HighlightedThinkingText text="t" className="custom" />,
    );
    expect(container.firstChild).toHaveClass('custom');
  });
});
