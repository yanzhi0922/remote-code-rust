import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Usage } from './Usage';

afterEach(() => {
  cleanup();
});

describe('Usage', () => {
  const stats = {
    totalTokens: 15000,
    inputTokens: 10000,
    outputTokens: 5000,
    totalCost: 1.23,
    sessionCount: 5,
    averageResponseTime: 250,
  };

  it('renders usage stats', () => {
    render(<Usage stats={stats} />);
    expect(screen.getByTestId('usage-panel')).toBeInTheDocument();
  });

  it('shows total tokens', () => {
    render(<Usage stats={stats} />);
    expect(screen.getByTestId('usage-total-tokens')).toHaveTextContent('15.0K');
  });

  it('shows cost', () => {
    render(<Usage stats={stats} />);
    expect(screen.getByTestId('usage-cost')).toHaveTextContent('$1.23');
  });

  it('shows session count', () => {
    render(<Usage stats={stats} />);
    expect(screen.getByTestId('usage-sessions')).toHaveTextContent('5');
  });

  it('shows token breakdown', () => {
    render(<Usage stats={stats} />);
    expect(screen.getByText('10.0K')).toBeInTheDocument();
    expect(screen.getByText('5.0K')).toBeInTheDocument();
  });
});
