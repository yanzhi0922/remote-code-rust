import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { RemoveWorkspaceDirectory } from './RemoveWorkspaceDirectory';

describe('RemoveWorkspaceDirectory', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<RemoveWorkspaceDirectory path="/home" onRemove={vi.fn()} />);
    expect(screen.getByTestId('remove-workspace-directory')).toBeInTheDocument();
  });

  it('shows path', () => {
    render(<RemoveWorkspaceDirectory path="/home/proj" onRemove={vi.fn()} />);
    expect(screen.getByText('/home/proj')).toBeInTheDocument();
  });

  it('calls onRemove', () => {
    const fn = vi.fn();
    render(<RemoveWorkspaceDirectory path="/p" onRemove={fn} />);
    fireEvent.click(screen.getByText('移除'));
    expect(fn).toHaveBeenCalled();
  });
});
