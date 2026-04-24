import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { BackgroundTaskStatus } from './BackgroundTaskStatus';

describe('BackgroundTaskStatus', () => {
  afterEach(cleanup);

  it('renders running status', () => {
    render(<BackgroundTaskStatus status="running" />);
    const el = screen.getByTestId('background-task-status');
    expect(el).toHaveTextContent('Running');
    expect(el.className).toContain('bg-green-50');
  });

  it('renders completed status', () => {
    render(<BackgroundTaskStatus status="completed" />);
    const el = screen.getByTestId('background-task-status');
    expect(el).toHaveTextContent('Completed');
    expect(el.className).toContain('text-green-700');
  });

  it('renders failed status with red styling', () => {
    render(<BackgroundTaskStatus status="failed" />);
    const el = screen.getByTestId('background-task-status');
    expect(el).toHaveTextContent('Failed');
    expect(el.className).toContain('bg-red-50');
  });

  it('renders pending status with grey styling', () => {
    render(<BackgroundTaskStatus status="pending" />);
    const el = screen.getByTestId('background-task-status');
    expect(el).toHaveTextContent('Pending');
    expect(el.className).toContain('bg-slate-100');
  });

  it('applies custom className', () => {
    render(<BackgroundTaskStatus status="running" className="extra" />);
    expect(screen.getByTestId('background-task-status').className).toContain('extra');
  });

  it('has spinner for running status', () => {
    const { container } = render(<BackgroundTaskStatus status="running" />);
    expect(container.querySelector('.animate-spin')).toBeInTheDocument();
  });
});
