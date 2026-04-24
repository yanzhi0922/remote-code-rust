import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { WorkerPendingPermission } from './WorkerPendingPermission';

describe('WorkerPendingPermission', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<WorkerPendingPermission workerName="w1" toolName="Bash" />);
    expect(screen.getByTestId('worker-pending-permission')).toBeInTheDocument();
  });

  it('shows worker name', () => {
    render(<WorkerPendingPermission workerName="agent-2" toolName="Read" />);
    expect(screen.getByText('agent-2')).toBeInTheDocument();
  });

  it('shows tool name', () => {
    render(<WorkerPendingPermission workerName="w" toolName="Write" />);
    expect(screen.getByText(/Write/)).toBeInTheDocument();
  });
});
