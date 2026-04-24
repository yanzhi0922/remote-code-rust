import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { TaskAssignmentMessage } from './TaskAssignmentMessage';

describe('TaskAssignmentMessage', () => {
  afterEach(cleanup);

  it('渲染任务分配消息', () => {
    render(
      <TaskAssignmentMessage
        taskId="task-1"
        assignedBy="alice"
        subject="实现用户认证"
      />,
    );
    expect(screen.getByTestId('task-assignment-message')).toBeInTheDocument();
    expect(screen.getByText('Task Assigned')).toBeInTheDocument();
    expect(screen.getByText('task-1')).toBeInTheDocument();
    expect(screen.getByText('实现用户认证')).toBeInTheDocument();
    expect(screen.getByText('Assigned by alice')).toBeInTheDocument();
  });

  it('显示任务描述', () => {
    render(
      <TaskAssignmentMessage
        taskId="task-2"
        assignedBy="bob"
        subject="修复 Bug"
        description="修复登录页面的验证问题"
      />,
    );
    expect(screen.getByText('修复登录页面的验证问题')).toBeInTheDocument();
  });

  it('无描述时不显示描述区域', () => {
    render(
      <TaskAssignmentMessage
        taskId="task-3"
        assignedBy="charlie"
        subject="简单任务"
      />,
    );
    expect(screen.getByText('简单任务')).toBeInTheDocument();
  });

  it('使用青色边框样式', () => {
    const { container } = render(
      <TaskAssignmentMessage
        taskId="task-4"
        assignedBy="alice"
        subject="test"
      />,
    );
    expect(container.firstChild).toHaveClass('border-cyan-300');
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <TaskAssignmentMessage
        taskId="task-5"
        assignedBy="alice"
        subject="test"
        className="task-custom"
      />,
    );
    expect(container.firstChild).toHaveClass('task-custom');
  });
});
