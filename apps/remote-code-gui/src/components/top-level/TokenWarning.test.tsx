import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { TokenWarning } from './TokenWarning';

describe('TokenWarning', () => {
  afterEach(() => { cleanup(); });

  it('returns null when usage is normal', () => {
    const { container } = render(
      <TokenWarning tokenUsage={100} maxTokens={1000} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('shows warning when usage >= 70%', () => {
    const { getByTestId, getByText } = render(
      <TokenWarning tokenUsage={750} maxTokens={1000} />,
    );
    expect(getByTestId('token-warning')).toBeInTheDocument();
    expect(getByText(/Token usage high/)).toBeInTheDocument();
  });

  it('shows critical when usage >= 90%', () => {
    const { getByTestId, getByText } = render(
      <TokenWarning tokenUsage={950} maxTokens={1000} />,
    );
    expect(getByTestId('token-warning')).toBeInTheDocument();
    expect(getByText(/Token usage critical/)).toBeInTheDocument();
  });

  it('shows model name when provided', () => {
    const { getByText } = render(
      <TokenWarning tokenUsage={800} maxTokens={1000} model="gpt-4" />,
    );
    expect(getByText(/gpt-4/)).toBeInTheDocument();
  });
});
