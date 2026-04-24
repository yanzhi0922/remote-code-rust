import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { TeleportStashDialog } from './TeleportStashDialog';

afterEach(() => {
  cleanup();
});

describe('TeleportStashDialog', () => {
  it('renders with data-testid', () => {
    render(
      <TeleportStashDialog onStashAndContinue={vi.fn()} onCancel={vi.fn()} />,
    );
    expect(screen.getByTestId('teleport-stash-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(
      <TeleportStashDialog onStashAndContinue={vi.fn()} onCancel={vi.fn()} />,
    );
    expect(screen.getByText('Working Directory Has Changes')).toBeInTheDocument();
  });

  it('shows changed files', () => {
    render(
      <TeleportStashDialog
        onStashAndContinue={vi.fn()}
        onCancel={vi.fn()}
        changedFiles={['src/app.tsx', 'README.md']}
      />,
    );
    expect(screen.getByText('src/app.tsx')).toBeInTheDocument();
    expect(screen.getByText('README.md')).toBeInTheDocument();
  });

  it('shows file count when many files', () => {
    const files = Array.from({ length: 10 }, (_, i) => `file-${i}.ts`);
    render(
      <TeleportStashDialog
        onStashAndContinue={vi.fn()}
        onCancel={vi.fn()}
        changedFiles={files}
      />,
    );
    expect(screen.getByText('10 files changed')).toBeInTheDocument();
  });

  it('calls onStashAndContinue when stash is clicked', () => {
    const onStashAndContinue = vi.fn();
    render(
      <TeleportStashDialog onStashAndContinue={onStashAndContinue} onCancel={vi.fn()} />,
    );
    fireEvent.click(screen.getByTestId('teleport-stash-confirm'));
    expect(onStashAndContinue).toHaveBeenCalledOnce();
  });

  it('calls onCancel when cancel is clicked', () => {
    const onCancel = vi.fn();
    render(
      <TeleportStashDialog onStashAndContinue={vi.fn()} onCancel={onCancel} />,
    );
    fireEvent.click(screen.getByTestId('teleport-stash-cancel'));
    expect(onCancel).toHaveBeenCalledOnce();
  });
});
