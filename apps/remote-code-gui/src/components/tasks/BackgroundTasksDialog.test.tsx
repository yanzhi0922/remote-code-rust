import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { BackgroundTasksDialog } from './BackgroundTasksDialog';

const tasks = [
  { id: 't1', name: 'Task 1', status: 'running', startedAt: '2026-01-01' },
  { id: 't2', name: 'Task 2', status: 'completed', startedAt: '2026-01-01' },
];

describe('BackgroundTasksDialog', () => {
  afterEach(cleanup);

  it('returns null when visible is false', () => {
    render(<BackgroundTasksDialog visible={false} tasks={tasks} onClose={vi.fn()} />);
    expect(screen.queryByTestId('background-tasks-dialog')).toBeNull();
  });

  it('renders dialog when visible is true', () => {
    render(<BackgroundTasksDialog visible={true} tasks={tasks} onClose={vi.fn()} />);
    expect(screen.getByTestId('background-tasks-dialog')).toBeInTheDocument();
  });

  it('shows task names', () => {
    render(<BackgroundTasksDialog visible={true} tasks={tasks} onClose={vi.fn()} />);
    expect(screen.getByText('Task 1')).toBeInTheDocument();
    expect(screen.getByText('Task 2')).toBeInTheDocument();
  });

  it('shows empty message when no tasks', () => {
    render(<BackgroundTasksDialog visible={true} tasks={[]} onClose={vi.fn()} />);
    expect(screen.getByText('暂无后台任务')).toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    render(<BackgroundTasksDialog visible={true} tasks={tasks} onClose={onClose} />);
    fireEvent.click(screen.getByTestId('dialog-close'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('calls onSelectTask when task is clicked', () => {
    const onSelectTask = vi.fn();
    render(
      <BackgroundTasksDialog
        visible={true}
        tasks={tasks}
        onClose={vi.fn()}
        onSelectTask={onSelectTask}
      />,
    );
    fireEvent.click(screen.getByText('Task 1'));
    expect(onSelectTask).toHaveBeenCalledWith('t1');
  });

  it('applies custom className', () => {
    render(
      <BackgroundTasksDialog
        visible={true}
        tasks={tasks}
        onClose={vi.fn()}
        className="my-dialog"
      />,
    );
    expect(screen.getByTestId('background-tasks-dialog').className).toContain('my-dialog');
  });
});
