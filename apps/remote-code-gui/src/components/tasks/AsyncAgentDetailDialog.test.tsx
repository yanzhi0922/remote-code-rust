import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AsyncAgentDetailDialog } from './AsyncAgentDetailDialog';

describe('AsyncAgentDetailDialog', () => {
  afterEach(cleanup);

  it('returns null when visible is false', () => {
    render(
      <AsyncAgentDetailDialog
        visible={false}
        agentName="Agent"
        taskId="t1"
        status="running"
        onClose={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('async-agent-detail')).toBeNull();
  });

  it('renders dialog when visible', () => {
    render(
      <AsyncAgentDetailDialog
        visible={true}
        agentName="CodeAgent"
        taskId="t1"
        status="running"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByTestId('async-agent-detail')).toBeInTheDocument();
    expect(screen.getByText('CodeAgent')).toBeInTheDocument();
  });

  it('shows task ID', () => {
    render(
      <AsyncAgentDetailDialog
        visible={true}
        agentName="Agent"
        taskId="task-abc-123"
        status="completed"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText('task-abc-123')).toBeInTheDocument();
  });

  it('shows output when provided', () => {
    render(
      <AsyncAgentDetailDialog
        visible={true}
        agentName="Agent"
        taskId="t1"
        status="completed"
        output="Build succeeded"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText('Build succeeded')).toBeInTheDocument();
  });

  it('hides output section when not provided', () => {
    render(
      <AsyncAgentDetailDialog
        visible={true}
        agentName="Agent"
        taskId="t1"
        status="running"
        onClose={vi.fn()}
      />,
    );
    expect(screen.queryByText('输出')).toBeNull();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    render(
      <AsyncAgentDetailDialog
        visible={true}
        agentName="Agent"
        taskId="t1"
        status="running"
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByLabelText('关闭'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('applies custom className', () => {
    render(
      <AsyncAgentDetailDialog
        visible={true}
        agentName="Agent"
        taskId="t1"
        status="running"
        onClose={vi.fn()}
        className="custom-cls"
      />,
    );
    expect(screen.getByTestId('async-agent-detail').className).toContain('custom-cls');
  });
});
