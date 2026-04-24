import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { WorkspaceTab } from './WorkspaceTab';

describe('WorkspaceTab', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<WorkspaceTab directories={[]} onRemove={vi.fn()} />);
    expect(screen.getByTestId('workspace-tab')).toBeInTheDocument();
  });

  it('shows empty message', () => {
    render(<WorkspaceTab directories={[]} onRemove={vi.fn()} />);
    expect(screen.getByText('暂无工作区目录')).toBeInTheDocument();
  });

  it('shows directories', () => {
    render(<WorkspaceTab directories={['/home/proj', '/tmp']} onRemove={vi.fn()} />);
    expect(screen.getByText('/home/proj')).toBeInTheDocument();
    expect(screen.getByText('/tmp')).toBeInTheDocument();
  });

  it('calls onRemove', () => {
    const fn = vi.fn();
    render(<WorkspaceTab directories={['/path']} onRemove={fn} />);
    fireEvent.click(screen.getByText('移除'));
    expect(fn).toHaveBeenCalledWith('/path');
  });
});
