import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ShellDetailDialog } from './ShellDetailDialog';

describe('ShellDetailDialog', () => {
  afterEach(cleanup);

  it('returns null when visible is false', () => {
    render(
      <ShellDetailDialog
        visible={false}
        command="ls"
        output="files"
        onClose={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('shell-detail-dialog')).toBeNull();
  });

  it('renders dialog with command and output', () => {
    render(
      <ShellDetailDialog
        visible={true}
        command="npm test"
        output="all tests passed"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByTestId('shell-detail-dialog')).toBeInTheDocument();
    expect(screen.getByText('npm test')).toBeInTheDocument();
    expect(screen.getByText('all tests passed')).toBeInTheDocument();
  });

  it('shows exit code when provided', () => {
    render(
      <ShellDetailDialog
        visible={true}
        command="cmd"
        output="out"
        exitCode={1}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText(/退出码: 1/)).toBeInTheDocument();
  });

  it('shows duration when provided', () => {
    render(
      <ShellDetailDialog
        visible={true}
        command="cmd"
        output="out"
        duration={3500}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText(/耗时: 3s/)).toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    render(
      <ShellDetailDialog
        visible={true}
        command="cmd"
        output="out"
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByLabelText('关闭'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('applies custom className', () => {
    render(
      <ShellDetailDialog
        visible={true}
        command="cmd"
        output="out"
        onClose={vi.fn()}
        className="my-cls"
      />,
    );
    expect(screen.getByTestId('shell-detail-dialog').className).toContain('my-cls');
  });

  it('shows red exit code for non-zero', () => {
    render(
      <ShellDetailDialog
        visible={true}
        command="cmd"
        output="out"
        exitCode={1}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText(/退出码: 1/).className).toContain('text-red-600');
  });
});
