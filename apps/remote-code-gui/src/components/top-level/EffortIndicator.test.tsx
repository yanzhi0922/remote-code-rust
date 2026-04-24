import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { EffortIndicator } from './EffortIndicator';

afterEach(() => {
  cleanup();
});

describe('EffortIndicator', () => {
  it('renders effort level', () => {
    render(<EffortIndicator level="high" />);
    expect(screen.getByTestId('effort-indicator')).toBeInTheDocument();
    expect(screen.getByText('high')).toBeInTheDocument();
  });

  it('hides label when showLabel is false', () => {
    render(<EffortIndicator level="medium" showLabel={false} />);
    expect(screen.queryByText('medium')).not.toBeInTheDocument();
  });

  it('renders all levels', () => {
    for (const level of ['low', 'medium', 'high', 'max'] as const) {
      cleanup();
      render(<EffortIndicator level={level} />);
      expect(screen.getByText(level)).toBeInTheDocument();
    }
  });
});
