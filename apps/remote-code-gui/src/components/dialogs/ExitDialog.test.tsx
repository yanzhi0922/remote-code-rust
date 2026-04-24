import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { ExitDialog } from './ExitDialog';

afterEach(() => {
  cleanup();
});

describe('ExitDialog', () => {
  it('renders when showWorktree is true', () => {
    render(<ExitDialog onDone={vi.fn()} showWorktree />);
    expect(screen.getByTestId('exit-dialog')).toBeInTheDocument();
  });

  it('returns null when showWorktree is false', () => {
    const { container } = render(<ExitDialog onDone={vi.fn()} showWorktree={false} />);
    expect(container.innerHTML).toBe('');
  });

  it('shows title', () => {
    render(<ExitDialog onDone={vi.fn()} showWorktree />);
    expect(screen.getByText('Exit Session')).toBeInTheDocument();
  });

  it('calls onDone with goodbye message when Exit is clicked', () => {
    const onDone = vi.fn();
    render(<ExitDialog onDone={onDone} showWorktree />);
    fireEvent.click(screen.getByTestId('exit-dialog-confirm'));
    expect(onDone).toHaveBeenCalled();
    const message = onDone.mock.calls[0][0];
    expect(['Goodbye!', 'See ya!', 'Bye!', 'Catch you later!']).toContain(message);
  });

  it('shows cancel button when onCancel is provided', () => {
    render(<ExitDialog onDone={vi.fn()} onCancel={vi.fn()} showWorktree />);
    expect(screen.getByTestId('exit-dialog-cancel')).toBeInTheDocument();
  });

  it('calls onCancel when cancel is clicked', () => {
    const onCancel = vi.fn();
    render(<ExitDialog onDone={vi.fn()} onCancel={onCancel} showWorktree />);
    fireEvent.click(screen.getByTestId('exit-dialog-cancel'));
    expect(onCancel).toHaveBeenCalledOnce();
  });
});
