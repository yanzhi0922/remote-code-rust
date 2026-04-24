import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { FallbackToolUseErrorMessage } from './FallbackToolUseErrorMessage';

afterEach(() => {
  cleanup();
});

describe('FallbackToolUseErrorMessage', () => {
  it('renders error message', () => {
    render(<FallbackToolUseErrorMessage error="Something went wrong" />);
    expect(screen.getByTestId('fallback-tool-use-error')).toBeInTheDocument();
    expect(screen.getByTestId('fallback-tool-use-error-text')).toHaveTextContent('Error: Something went wrong');
  });

  it('preserves Error: prefix', () => {
    render(<FallbackToolUseErrorMessage error="Error: already prefixed" />);
    expect(screen.getByTestId('fallback-tool-use-error-text')).toHaveTextContent('Error: already prefixed');
  });

  it('shows truncation notice', () => {
    const longError = Array.from({ length: 15 }, (_, i) => `Line ${i}`).join('\n');
    render(<FallbackToolUseErrorMessage error={longError} />);
    expect(screen.getByText(/还有 5 行未显示/)).toBeInTheDocument();
  });

  it('does not truncate in verbose mode', () => {
    const longError = Array.from({ length: 15 }, (_, i) => `Line ${i}`).join('\n');
    render(<FallbackToolUseErrorMessage error={longError} verbose={true} />);
    expect(screen.queryByText(/未显示/)).not.toBeInTheDocument();
  });
});
