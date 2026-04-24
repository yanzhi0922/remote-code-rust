import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { BridgeDialog } from './BridgeDialog';

afterEach(() => {
  cleanup();
});

describe('BridgeDialog', () => {
  it('renders with data-testid', () => {
    render(<BridgeDialog onDone={vi.fn()} />);
    expect(screen.getByTestId('bridge-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<BridgeDialog onDone={vi.fn()} />);
    expect(screen.getByText('IDE Bridge')).toBeInTheDocument();
  });

  it('shows connected status', () => {
    render(<BridgeDialog onDone={vi.fn()} connected />);
    expect(screen.getByText('Connected')).toBeInTheDocument();
  });

  it('shows error message', () => {
    render(<BridgeDialog onDone={vi.fn()} error="Connection failed" />);
    expect(screen.getByText('Connection failed')).toBeInTheDocument();
  });

  it('shows repository info', () => {
    render(<BridgeDialog onDone={vi.fn()} repoName="my-repo" branchName="main" />);
    expect(screen.getByText(/my-repo/)).toBeInTheDocument();
    expect(screen.getByText(/main/)).toBeInTheDocument();
  });

  it('calls onDone when Done button is clicked', () => {
    const onDone = vi.fn();
    render(<BridgeDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('bridge-dialog-done'));
    expect(onDone).toHaveBeenCalledOnce();
  });

  it('calls onDone when close button is clicked', () => {
    const onDone = vi.fn();
    render(<BridgeDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('bridge-dialog-close'));
    expect(onDone).toHaveBeenCalledOnce();
  });
});
