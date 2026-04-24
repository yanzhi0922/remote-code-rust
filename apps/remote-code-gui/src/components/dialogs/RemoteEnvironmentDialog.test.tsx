import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { RemoteEnvironmentDialog } from './RemoteEnvironmentDialog';

afterEach(() => {
  cleanup();
});

describe('RemoteEnvironmentDialog', () => {
  const environments = [
    { environment_id: 'env-1', name: 'Production', description: 'Prod env' },
    { environment_id: 'env-2', name: 'Staging', description: 'Staging env' },
  ];

  it('renders with data-testid', () => {
    render(<RemoteEnvironmentDialog onDone={vi.fn()} environments={environments} />);
    expect(screen.getByTestId('remote-environment-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<RemoteEnvironmentDialog onDone={vi.fn()} environments={environments} />);
    expect(screen.getByText('Select Remote Environment')).toBeInTheDocument();
  });

  it('shows environment options', () => {
    render(<RemoteEnvironmentDialog onDone={vi.fn()} environments={environments} />);
    expect(screen.getByText('Production')).toBeInTheDocument();
    expect(screen.getByText('Staging')).toBeInTheDocument();
  });

  it('shows loading state', () => {
    render(<RemoteEnvironmentDialog onDone={vi.fn()} loading />);
    expect(screen.getByText('Loading environments…')).toBeInTheDocument();
  });

  it('calls onDone when cancel is clicked', () => {
    const onDone = vi.fn();
    render(<RemoteEnvironmentDialog onDone={onDone} environments={environments} />);
    fireEvent.click(screen.getByTestId('remote-environment-cancel'));
    expect(onDone).toHaveBeenCalled();
  });

  it('calls onDone when close is clicked', () => {
    const onDone = vi.fn();
    render(<RemoteEnvironmentDialog onDone={onDone} environments={environments} />);
    fireEvent.click(screen.getByTestId('remote-environment-close'));
    expect(onDone).toHaveBeenCalledWith();
  });
});
