import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { ClaudeMdExternalIncludesDialog } from './ClaudeMdExternalIncludesDialog';

afterEach(() => {
  cleanup();
});

describe('ClaudeMdExternalIncludesDialog', () => {
  it('renders with data-testid', () => {
    render(<ClaudeMdExternalIncludesDialog onDone={vi.fn()} />);
    expect(screen.getByTestId('claude-md-external-includes-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<ClaudeMdExternalIncludesDialog onDone={vi.fn()} />);
    expect(screen.getByText('External Imports Detected')).toBeInTheDocument();
  });

  it('shows standalone title when isStandaloneDialog', () => {
    render(<ClaudeMdExternalIncludesDialog onDone={vi.fn()} isStandaloneDialog />);
    expect(screen.getByText('CLAUDE.md External Includes')).toBeInTheDocument();
  });

  it('shows external includes when provided', () => {
    render(
      <ClaudeMdExternalIncludesDialog
        onDone={vi.fn()}
        externalIncludes={[{ path: '/foo/bar.md', source: 'project' }]}
      />,
    );
    expect(screen.getByText(/\/foo\/bar\.md/)).toBeInTheDocument();
  });

  it('calls onDone when Yes is clicked', () => {
    const onDone = vi.fn();
    render(<ClaudeMdExternalIncludesDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('claude-md-external-yes'));
    expect(onDone).toHaveBeenCalledOnce();
  });

  it('calls onDone when No is clicked', () => {
    const onDone = vi.fn();
    render(<ClaudeMdExternalIncludesDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('claude-md-external-no'));
    expect(onDone).toHaveBeenCalledOnce();
  });
});
