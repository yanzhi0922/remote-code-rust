import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { BackgroundTask } from './BackgroundTask';

const baseTask = {
  id: 't1',
  name: 'Build Project',
  status: 'running' as const,
  startedAt: '2026-01-01T00:00:00Z',
};

describe('BackgroundTask', () => {
  afterEach(cleanup);

  it('renders task name', () => {
    render(<BackgroundTask task={baseTask} />);
    expect(screen.getByTestId('background-task')).toHaveTextContent('Build Project');
  });

  it('shows running status with green styling', () => {
    render(<BackgroundTask task={baseTask} />);
    const el = screen.getByTestId('background-task');
    expect(el.className).toContain('border-green-300');
    expect(el).toHaveTextContent('Running');
  });

  it('shows failed status with red styling', () => {
    render(<BackgroundTask task={{ ...baseTask, status: 'failed' }} />);
    const el = screen.getByTestId('background-task');
    expect(el.className).toContain('border-red-300');
    expect(el).toHaveTextContent('Failed');
  });

  it('shows completed status', () => {
    render(<BackgroundTask task={{ ...baseTask, status: 'completed' }} />);
    expect(screen.getByTestId('background-task')).toHaveTextContent('Completed');
  });

  it('shows pending status', () => {
    render(<BackgroundTask task={{ ...baseTask, status: 'pending' }} />);
    expect(screen.getByTestId('background-task')).toHaveTextContent('Pending');
  });

  it('renders progress bar when progress provided', () => {
    const { container } = render(
      <BackgroundTask task={{ ...baseTask, progress: 60 }} />,
    );
    const bar = container.querySelector('[style*="width: 60%"]');
    expect(bar).toBeInTheDocument();
  });

  it('calls onClick when clicked', () => {
    const onClick = vi.fn();
    render(<BackgroundTask task={baseTask} onClick={onClick} />);
    fireEvent.click(screen.getByTestId('background-task'));
    expect(onClick).toHaveBeenCalledOnce();
  });

  it('applies custom className', () => {
    render(<BackgroundTask task={baseTask} className="my-task" />);
    expect(screen.getByTestId('background-task').className).toContain('my-task');
  });
});
