import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { IdeAutoConnectDialog } from './IdeAutoConnectDialog';

afterEach(() => {
  cleanup();
});

describe('IdeAutoConnectDialog', () => {
  it('renders with data-testid', () => {
    render(<IdeAutoConnectDialog onComplete={vi.fn()} />);
    expect(screen.getByTestId('ide-auto-connect-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<IdeAutoConnectDialog onComplete={vi.fn()} />);
    expect(screen.getByText('Auto-Connect to IDE')).toBeInTheDocument();
  });

  it('shows configuration hint', () => {
    render(<IdeAutoConnectDialog onComplete={vi.fn()} />);
    expect(screen.getByText(/\/config or with the --ide flag/)).toBeInTheDocument();
  });

  it('calls onComplete when Yes is clicked', () => {
    const onComplete = vi.fn();
    render(<IdeAutoConnectDialog onComplete={onComplete} />);
    fireEvent.click(screen.getByTestId('ide-auto-connect-yes'));
    expect(onComplete).toHaveBeenCalledOnce();
  });

  it('calls onComplete when No is clicked', () => {
    const onComplete = vi.fn();
    render(<IdeAutoConnectDialog onComplete={onComplete} />);
    fireEvent.click(screen.getByTestId('ide-auto-connect-no'));
    expect(onComplete).toHaveBeenCalledOnce();
  });

  it('calls onComplete when close is clicked', () => {
    const onComplete = vi.fn();
    render(<IdeAutoConnectDialog onComplete={onComplete} />);
    fireEvent.click(screen.getByTestId('ide-auto-connect-close'));
    expect(onComplete).toHaveBeenCalledOnce();
  });
});
