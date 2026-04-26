import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { TaskTree, type TaskItemData } from './TaskTree';

afterEach(() => { cleanup(); });

describe('TaskTree', () => {
  it('renders nothing when no tasks', () => {
    const { container } = render(<TaskTree />);
    expect(container.innerHTML).toBe('');
  });

  it('renders task summary header', () => {
    const tasks: TaskItemData[] = [
      { name: 'Read file', status: 'completed' },
      { name: 'Write file', status: 'running' },
    ];
    render(<TaskTree toolCalls={tasks} />);
    expect(screen.getByText(/共 2 个任务/)).toBeInTheDocument();
    expect(screen.getByText(/已完成 1 个/)).toBeInTheDocument();
  });

  it('renders task names', () => {
    const tasks: TaskItemData[] = [
      { name: 'Read file', status: 'completed' },
      { name: 'Edit code', status: 'pending' },
    ];
    render(<TaskTree toolCalls={tasks} />);
    expect(screen.getByText('Read file')).toBeInTheDocument();
    expect(screen.getByText('Edit code')).toBeInTheDocument();
  });

  it('renders empty toolCalls array as nothing', () => {
    const { container } = render(<TaskTree toolCalls={[]} />);
    expect(container.innerHTML).toBe('');
  });
});
