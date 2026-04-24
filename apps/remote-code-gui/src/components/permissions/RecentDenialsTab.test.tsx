import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { RecentDenialsTab } from './RecentDenialsTab';

describe('RecentDenialsTab', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<RecentDenialsTab denials={[]} />);
    expect(screen.getByTestId('recent-denials-tab')).toBeInTheDocument();
  });

  it('shows empty message', () => {
    render(<RecentDenialsTab denials={[]} />);
    expect(screen.getByText('暂无拒绝记录')).toBeInTheDocument();
  });

  it('shows denial records', () => {
    render(<RecentDenialsTab denials={[{ toolName: 'Bash', reason: 'dangerous', timestamp: '10:30' }]} />);
    expect(screen.getByText('Bash')).toBeInTheDocument();
    expect(screen.getByText('dangerous')).toBeInTheDocument();
  });
});
