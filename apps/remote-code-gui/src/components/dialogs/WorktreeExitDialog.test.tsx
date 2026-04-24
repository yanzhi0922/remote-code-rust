import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { WorktreeExitDialog } from './WorktreeExitDialog';

afterEach(() => {
  cleanup();
});

describe('WorktreeExitDialog', () => {
  it('renders with data-testid', () => {
    render(<WorktreeExitDialog onDone={vi.fn()} />);
    expect(screen.getByTestId('worktree-exit-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<WorktreeExitDialog onDone={vi.fn()} />);
    expect(screen.getByText('Exit Worktree')).toBeInTheDocument();
  });

  it('shows changed files', () => {
    render(<WorktreeExitDialog onDone={vi.fn()} changes={['src/app.tsx', 'lib/utils.ts']} />);
    expect(screen.getByText('src/app.tsx')).toBeInTheDocument();
    expect(screen.getByText('lib/utils.ts')).toBeInTheDocument();
  });

  it('shows commit count', () => {
    render(<WorktreeExitDialog onDone={vi.fn()} commitCount={3} />);
    expect(screen.getByText(/3 commits to eject/)).toBeInTheDocument();
  });

  it('calls onDone when keep is clicked', () => {
    const onDone = vi.fn();
    render(<WorktreeExitDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('worktree-exit-keep'));
    expect(onDone).toHaveBeenCalledWith('Worktree kept');
  });

  it('calls onDone when remove is clicked', async () => {
    const onDone = vi.fn();
    render(<WorktreeExitDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('worktree-exit-remove'));
    // Wait for the async timeout to complete
    await new Promise((r) => setTimeout(r, 600));
    expect(onDone).toHaveBeenCalledWith('Worktree removed');
  });
});
