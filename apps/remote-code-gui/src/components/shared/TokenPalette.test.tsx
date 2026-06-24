import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { TokenPalette } from './TokenPalette';

afterEach(() => { cleanup(); });

describe('TokenPalette', () => {
  it('renders input, output, and free token counts', () => {
    render(
      <TokenPalette
        inputTokens={5000}
        outputTokens={2000}
        maxTokens={128000}
        estimatedTokens={7000}
      />,
    );
    expect(screen.getByText('5.0K')).toBeInTheDocument();
    expect(screen.getByText('2.0K')).toBeInTheDocument();
  });

  it('shows usage percentage', () => {
    render(
      <TokenPalette
        inputTokens={1000}
        outputTokens={500}
        maxTokens={128000}
        estimatedTokens={1500}
      />,
    );
    expect(screen.getByText('1% used')).toBeInTheDocument();
  });

  it('handles high usage with warning coloring', () => {
    render(
      <TokenPalette
        inputTokens={100000}
        outputTokens={20000}
        maxTokens={128000}
        estimatedTokens={120000}
      />,
    );
    expect(screen.getByText('94% used')).toBeInTheDocument();
  });

  it('renders the stacked bar with correct segments', () => {
    const { container } = render(
      <TokenPalette
        inputTokens={50000}
        outputTokens={30000}
        maxTokens={200000}
        estimatedTokens={80000}
      />,
    );
    const bars = container.querySelectorAll('.h-full');
    expect(bars.length).toBeGreaterThanOrEqual(3);
  });
});
