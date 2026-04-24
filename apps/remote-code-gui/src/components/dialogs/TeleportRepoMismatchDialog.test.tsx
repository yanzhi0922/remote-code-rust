import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { TeleportRepoMismatchDialog } from './TeleportRepoMismatchDialog';

afterEach(() => {
  cleanup();
});

describe('TeleportRepoMismatchDialog', () => {
  it('renders with data-testid', () => {
    render(
      <TeleportRepoMismatchDialog
        targetRepo="owner/repo"
        initialPaths={['/path/a', '/path/b']}
        onSelectPath={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByTestId('teleport-repo-mismatch-dialog')).toBeInTheDocument();
  });

  it('shows target repo', () => {
    render(
      <TeleportRepoMismatchDialog
        targetRepo="owner/repo"
        initialPaths={['/path/a']}
        onSelectPath={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByText('owner/repo')).toBeInTheDocument();
  });

  it('shows path options', () => {
    render(
      <TeleportRepoMismatchDialog
        targetRepo="owner/repo"
        initialPaths={['/path/a', '/path/b']}
        onSelectPath={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByTestId('teleport-path-/path/a')).toBeInTheDocument();
    expect(screen.getByTestId('teleport-path-/path/b')).toBeInTheDocument();
  });

  it('calls onSelectPath when a path is clicked', () => {
    const onSelectPath = vi.fn();
    render(
      <TeleportRepoMismatchDialog
        targetRepo="owner/repo"
        initialPaths={['/path/a']}
        onSelectPath={onSelectPath}
        onCancel={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('teleport-path-/path/a'));
    expect(onSelectPath).toHaveBeenCalledWith('/path/a');
  });

  it('calls onCancel when cancel is clicked', () => {
    const onCancel = vi.fn();
    render(
      <TeleportRepoMismatchDialog
        targetRepo="owner/repo"
        initialPaths={['/path/a']}
        onSelectPath={vi.fn()}
        onCancel={onCancel}
      />,
    );
    fireEvent.click(screen.getByTestId('teleport-repo-mismatch-cancel'));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('shows fallback when no paths available', () => {
    render(
      <TeleportRepoMismatchDialog
        targetRepo="owner/repo"
        initialPaths={[]}
        onSelectPath={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByText(/Run claude --teleport/)).toBeInTheDocument();
  });
});
