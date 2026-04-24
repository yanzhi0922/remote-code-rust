import { afterEach, describe, it, expect } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { TaskListV2 } from './TaskListV2';

afterEach(() => {
  cleanup();
});

const tasks = [
  { id: '1', title: 'Task 1', status: 'completed' as const, owner: undefined, blockedBy: [] },
  { id: '2', title: 'Task 2', status: 'in_progress' as const, owner: 'agent-1', blockedBy: [] },
  { id: '3', title: 'Task 3', status: 'pending' as const, owner: undefined, blockedBy: ['2'] },
];

describe('TaskListV2', () => {
  it('renders task items', () => {
    render(<TaskListV2 tasks={tasks} />);
    expect(screen.getByTestId('task-list-v2')).toBeInTheDocument();
    expect(screen.getByTestId('task-item-1')).toBeInTheDocument();
    expect(screen.getByTestId('task-item-2')).toBeInTheDocument();
    expect(screen.getByTestId('task-item-3')).toBeInTheDocument();
  });

  it('returns null for empty tasks', () => {
    const { container } = render(<TaskListV2 tasks={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('shows task titles', () => {
    render(<TaskListV2 tasks={tasks} />);
    expect(screen.getByText('Task 1')).toBeInTheDocument();
    expect(screen.getByText('Task 2')).toBeInTheDocument();
  });

  it('shows owner when present', () => {
    render(<TaskListV2 tasks={tasks} />);
    expect(screen.getByText('@agent-1')).toBeInTheDocument();
  });

  it('renders standalone mode with counts', () => {
    render(<TaskListV2 tasks={tasks} isStandalone />);
    expect(screen.getByTestId('task-list-v2')).toBeInTheDocument();
    // Check that the summary line contains the expected counts
    const summary = screen.getByTestId('task-list-v2').parentElement;
    expect(summary?.textContent).toContain('3');
    expect(summary?.textContent).toContain('done');
  });

  it('applies opacity to completed tasks', () => {
    render(<TaskListV2 tasks={tasks} />);
    const completedItem = screen.getByTestId('task-item-1');
    expect(completedItem.classList.contains('opacity-60')).toBe(true);
  });
});
