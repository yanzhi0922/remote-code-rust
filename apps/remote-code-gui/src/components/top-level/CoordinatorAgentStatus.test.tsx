import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { CoordinatorAgentStatus } from './CoordinatorAgentStatus';

afterEach(() => {
  cleanup();
});

const tasks = [
  {
    id: 'task-1',
    name: 'researcher',
    status: 'in_progress' as const,
    description: 'Analyzing codebase',
    startTime: Date.now() - 60000,
    tokenCount: 500,
  },
  {
    id: 'task-2',
    name: 'writer',
    status: 'completed' as const,
    description: 'Writing docs',
    startTime: Date.now() - 120000,
    endTime: Date.now() - 30000,
  },
];

describe('CoordinatorAgentStatus', () => {
  it('returns null for empty tasks', () => {
    const { container } = render(<CoordinatorAgentStatus tasks={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders task list', () => {
    render(<CoordinatorAgentStatus tasks={tasks} />);
    expect(screen.getByTestId('coordinator-agent-status')).toBeInTheDocument();
    expect(screen.getByTestId('coordinator-task-task-1')).toBeInTheDocument();
    expect(screen.getByTestId('coordinator-task-task-2')).toBeInTheDocument();
  });

  it('shows main button', () => {
    render(<CoordinatorAgentStatus tasks={tasks} />);
    expect(screen.getByTestId('coordinator-main')).toBeInTheDocument();
  });

  it('shows task descriptions', () => {
    render(<CoordinatorAgentStatus tasks={tasks} />);
    expect(screen.getByText(/researcher:/)).toBeInTheDocument();
    expect(screen.getByText(/writer:/)).toBeInTheDocument();
  });

  it('calls onTaskClick when task is clicked', () => {
    const onTaskClick = vi.fn();
    render(<CoordinatorAgentStatus tasks={tasks} onTaskClick={onTaskClick} />);
    fireEvent.click(screen.getByTestId('coordinator-task-task-1'));
    expect(onTaskClick).toHaveBeenCalledWith('task-1');
  });

  it('calls onMainClick when main is clicked', () => {
    const onMainClick = vi.fn();
    render(<CoordinatorAgentStatus tasks={tasks} onMainClick={onMainClick} />);
    fireEvent.click(screen.getByTestId('coordinator-main'));
    expect(onMainClick).toHaveBeenCalled();
  });

  it('shows token count when available', () => {
    render(<CoordinatorAgentStatus tasks={tasks} />);
    expect(screen.getByText(/500 tokens/)).toBeInTheDocument();
  });
});
