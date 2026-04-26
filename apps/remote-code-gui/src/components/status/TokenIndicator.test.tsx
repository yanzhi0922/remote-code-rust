import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { TokenIndicator } from './TokenIndicator';

describe('TokenIndicator', () => {
  afterEach(() => { cleanup(); });

  it('renders token counts', () => {
    const { getByTestId } = render(
      <TokenIndicator usage={{ inputTokens: 1000, outputTokens: 500, totalTokens: 1500 }} />,
    );
    expect(getByTestId('token-indicator')).toBeInTheDocument();
    expect(getByTestId('input-tokens')).toBeInTheDocument();
    expect(getByTestId('output-tokens')).toBeInTheDocument();
    expect(getByTestId('total-tokens')).toBeInTheDocument();
  });

  it('formats large token values', () => {
    const { getByTestId } = render(
      <TokenIndicator usage={{ inputTokens: 1500000, outputTokens: 500000, totalTokens: 2000000 }} />,
    );
    expect(getByTestId('input-tokens').textContent).toBe('1.5M');
    expect(getByTestId('output-tokens').textContent).toBe('500.0K');
    expect(getByTestId('total-tokens').textContent).toBe('2.0M');
  });

  it('shows progress bar when maxTokens provided', () => {
    const { getByTestId } = render(
      <TokenIndicator
        usage={{ inputTokens: 500, outputTokens: 300, totalTokens: 800, maxTokens: 1000 }}
      />,
    );
    expect(getByTestId('token-progress')).toBeInTheDocument();
  });

  it('does not show progress bar when maxTokens not provided', () => {
    const { queryByTestId } = render(
      <TokenIndicator usage={{ inputTokens: 500, outputTokens: 300, totalTokens: 800 }} />,
    );
    expect(queryByTestId('token-progress')).not.toBeInTheDocument();
  });
});
