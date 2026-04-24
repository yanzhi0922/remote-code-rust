import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { TeleportResumeDialog } from './TeleportResumeDialog';

afterEach(() => {
  cleanup();
});

describe('TeleportResumeDialog', () => {
  const sessions = [
    { id: 'sess-1', title: 'Feature work', createdAt: '2024-01-01' },
    { id: 'sess-2', title: 'Bug fix' },
  ];

  it('renders with data-testid', () => {
    render(<TeleportResumeDialog onComplete={vi.fn()} onCancel={vi.fn()} sessions={sessions} />);
    expect(screen.getByTestId('teleport-resume-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<TeleportResumeDialog onComplete={vi.fn()} onCancel={vi.fn()} sessions={sessions} />);
    expect(screen.getByText('Resume Session')).toBeInTheDocument();
  });

  it('shows session options', () => {
    render(<TeleportResumeDialog onComplete={vi.fn()} onCancel={vi.fn()} sessions={sessions} />);
    expect(screen.getByText('Feature work')).toBeInTheDocument();
    expect(screen.getByText('Bug fix')).toBeInTheDocument();
  });

  it('shows loading state', () => {
    render(<TeleportResumeDialog onComplete={vi.fn()} onCancel={vi.fn()} loading />);
    expect(screen.getByText('Loading sessions…')).toBeInTheDocument();
  });

  it('calls onCancel when cancel is clicked', () => {
    const onCancel = vi.fn();
    render(<TeleportResumeDialog onComplete={vi.fn()} onCancel={onCancel} sessions={sessions} />);
    fireEvent.click(screen.getByTestId('teleport-resume-cancel'));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('shows error message', () => {
    render(
      <TeleportResumeDialog
        onComplete={vi.fn()}
        onCancel={vi.fn()}
        sessions={sessions}
        error="Failed to load"
      />,
    );
    expect(screen.getByText('Failed to load')).toBeInTheDocument();
  });
});
