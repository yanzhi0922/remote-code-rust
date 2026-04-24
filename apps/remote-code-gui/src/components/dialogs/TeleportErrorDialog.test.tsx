import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { TeleportErrorDialog } from './TeleportErrorDialog';

afterEach(() => {
  cleanup();
});

describe('TeleportErrorDialog', () => {
  it('renders with needsLogin error', () => {
    render(<TeleportErrorDialog onComplete={vi.fn()} onCancel={vi.fn()} errorType="needsLogin" />);
    expect(screen.getByTestId('teleport-error-dialog')).toBeInTheDocument();
  });

  it('renders with needsGitStash error', () => {
    render(<TeleportErrorDialog onComplete={vi.fn()} onCancel={vi.fn()} errorType="needsGitStash" />);
    expect(screen.getByTestId('teleport-error-dialog')).toBeInTheDocument();
  });

  it('returns null when no error', () => {
    const { container } = render(
      <TeleportErrorDialog onComplete={vi.fn()} onCancel={vi.fn()} errorType={null} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('calls onComplete when login button is clicked', () => {
    const onComplete = vi.fn();
    render(<TeleportErrorDialog onComplete={onComplete} onCancel={vi.fn()} errorType="needsLogin" />);
    fireEvent.click(screen.getByTestId('teleport-error-login'));
    expect(onComplete).toHaveBeenCalledOnce();
  });

  it('calls onCancel when cancel button is clicked', () => {
    const onCancel = vi.fn();
    render(<TeleportErrorDialog onComplete={vi.fn()} onCancel={onCancel} errorType="needsLogin" />);
    fireEvent.click(screen.getByTestId('teleport-error-cancel-login'));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('calls onComplete when stash button is clicked', () => {
    const onComplete = vi.fn();
    render(<TeleportErrorDialog onComplete={onComplete} onCancel={vi.fn()} errorType="needsGitStash" />);
    fireEvent.click(screen.getByTestId('teleport-error-stash'));
    expect(onComplete).toHaveBeenCalledOnce();
  });
});
