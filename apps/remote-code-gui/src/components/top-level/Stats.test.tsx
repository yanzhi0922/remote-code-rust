import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { Stats, type StatsData } from './Stats';

describe('Stats', () => {
  afterEach(() => { cleanup(); });

  it('shows loading state', () => {
    const { getByTestId, getByText } = render(<Stats loading />);
    expect(getByTestId('stats-loading')).toBeInTheDocument();
    expect(getByText(/Loading stats/)).toBeInTheDocument();
  });

  it('shows empty state when stats is null', () => {
    const { getByTestId, getByText } = render(<Stats stats={null} />);
    expect(getByTestId('stats-empty')).toBeInTheDocument();
    expect(getByText(/No stats available/)).toBeInTheDocument();
  });

  it('renders stats data with sessions, tokens, cost', () => {
    const data: StatsData = {
      totalSessions: 10,
      totalTokens: 5000,
      totalCost: 1.23,
      modelsUsed: {},
    };
    const { getByTestId, getByText } = render(<Stats stats={data} />);
    expect(getByTestId('stats')).toBeInTheDocument();
    expect(getByText('10')).toBeInTheDocument();
    expect(getByText('$1.23')).toBeInTheDocument();
  });

  it('renders models used section', () => {
    const data: StatsData = {
      totalSessions: 5,
      totalTokens: 1000,
      totalCost: 0.5,
      modelsUsed: { 'gpt-4': 3, 'claude-3': 2 },
    };
    const { getByText } = render(<Stats stats={data} />);
    expect(getByText('gpt-4')).toBeInTheDocument();
    expect(getByText('claude-3')).toBeInTheDocument();
  });
});
