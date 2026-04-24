import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { ResumeTask } from './ResumeTask';

afterEach(() => {
  cleanup();
});

const sessions = [
  { id: 's1', title: 'Fix auth bug', updated_at: '2 hours ago' },
  { id: 's2', title: 'Add tests', updated_at: '1 day ago' },
];

describe('ResumeTask', () => {
  it('renders loading state', () => {
    render(<ResumeTask loading onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByTestId('resume-task')).toBeInTheDocument();
    expect(screen.getByText('Loading sessions…')).toBeInTheDocument();
  });

  it('renders error state', () => {
    render(
      <ResumeTask error="Network error" onSelect={vi.fn()} onCancel={vi.fn()} />,
    );
    expect(screen.getByText('Error loading sessions')).toBeInTheDocument();
    expect(screen.getByText('Network error')).toBeInTheDocument();
  });

  it('renders empty state', () => {
    render(<ResumeTask sessions={[]} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByText('No sessions found')).toBeInTheDocument();
  });

  it('renders session list', () => {
    render(<ResumeTask sessions={sessions} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByText('Fix auth bug')).toBeInTheDocument();
    expect(screen.getByText('Add tests')).toBeInTheDocument();
  });

  it('calls onSelect when session is clicked', () => {
    const onSelect = vi.fn();
    render(<ResumeTask sessions={sessions} onSelect={onSelect} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByTestId('resume-session-s1'));
    expect(onSelect).toHaveBeenCalledWith(sessions[0]);
  });

  it('calls onCancel when cancel is clicked', () => {
    const onCancel = vi.fn();
    render(<ResumeTask sessions={sessions} onSelect={vi.fn()} onCancel={onCancel} />);
    fireEvent.click(screen.getByTestId('resume-cancel'));
    expect(onCancel).toHaveBeenCalled();
  });
});
