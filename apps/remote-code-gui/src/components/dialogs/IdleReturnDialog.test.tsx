import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { IdleReturnDialog } from './IdleReturnDialog';

afterEach(() => {
  cleanup();
});

describe('IdleReturnDialog', () => {
  it('renders with data-testid', () => {
    render(<IdleReturnDialog idleMinutes={30} totalInputTokens={5000} onDone={vi.fn()} />);
    expect(screen.getByTestId('idle-return-dialog')).toBeInTheDocument();
  });

  it('shows idle duration in minutes', () => {
    render(<IdleReturnDialog idleMinutes={30} totalInputTokens={5000} onDone={vi.fn()} />);
    expect(screen.getByText(/30m/)).toBeInTheDocument();
  });

  it('shows idle duration in hours', () => {
    render(<IdleReturnDialog idleMinutes={120} totalInputTokens={5000} onDone={vi.fn()} />);
    expect(screen.getByText(/2h/)).toBeInTheDocument();
  });

  it('shows token count', () => {
    render(<IdleReturnDialog idleMinutes={30} totalInputTokens={5000} onDone={vi.fn()} />);
    expect(screen.getByText(/5,000/)).toBeInTheDocument();
  });

  it('calls onDone with continue when continue is clicked', () => {
    const onDone = vi.fn();
    render(<IdleReturnDialog idleMinutes={30} totalInputTokens={5000} onDone={onDone} />);
    fireEvent.click(screen.getByTestId('idle-return-continue'));
    expect(onDone).toHaveBeenCalledWith('continue');
  });

  it('calls onDone with dismiss when close is clicked', () => {
    const onDone = vi.fn();
    render(<IdleReturnDialog idleMinutes={30} totalInputTokens={5000} onDone={onDone} />);
    fireEvent.click(screen.getByTestId('idle-return-close'));
    expect(onDone).toHaveBeenCalledWith('dismiss');
  });
});
