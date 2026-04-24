import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { WorkerBadge } from './WorkerBadge';

describe('WorkerBadge', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<WorkerBadge name="agent-1" />);
    expect(screen.getByTestId('worker-badge')).toBeInTheDocument();
  });

  it('shows name', () => {
    render(<WorkerBadge name="worker-A" />);
    expect(screen.getByText('worker-A')).toBeInTheDocument();
  });

  it('applies busy color', () => {
    render(<WorkerBadge name="w" status="busy" />);
    expect(screen.getByTestId('worker-badge')).toHaveClass('bg-blue-100');
  });
});
