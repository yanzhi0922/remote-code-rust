import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AddWorkspaceDirectory } from './AddWorkspaceDirectory';

describe('AddWorkspaceDirectory', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<AddWorkspaceDirectory onAdd={vi.fn()} />);
    expect(screen.getByTestId('add-workspace-directory')).toBeInTheDocument();
  });

  it('calls onAdd when clicked', () => {
    const onAdd = vi.fn();
    render(<AddWorkspaceDirectory onAdd={onAdd} />);
    fireEvent.click(screen.getByTestId('add-workspace-directory'));
    expect(onAdd).toHaveBeenCalledTimes(1);
  });

  it('applies custom className', () => {
    const { container } = render(
      <AddWorkspaceDirectory onAdd={vi.fn()} className="custom" />,
    );
    expect(container.firstChild).toHaveClass('custom');
  });
});
