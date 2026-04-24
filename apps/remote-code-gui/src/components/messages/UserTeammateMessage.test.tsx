import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserTeammateMessage } from './UserTeammateMessage';

describe('UserTeammateMessage', () => {
  afterEach(cleanup);

  it('渲染团队成员消息', () => {
    render(
      <UserTeammateMessage text="子任务已完成" senderName="Worker-1" />,
    );
    expect(screen.getByText('Worker-1')).toBeInTheDocument();
    expect(screen.getByText('子任务已完成')).toBeInTheDocument();
  });

  it('空文本返回 null', () => {
    const { container } = render(
      <UserTeammateMessage text="  " senderName="Worker-1" />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('显示角色标签', () => {
    render(
      <UserTeammateMessage
        text="处理中"
        senderName="Worker-2"
        senderRole="worker"
      />,
    );
    expect(screen.getByText('worker')).toBeInTheDocument();
  });

  it('无角色时不显示角色标签', () => {
    render(<UserTeammateMessage text="完成" senderName="Agent" />);
    expect(screen.queryByText('worker')).not.toBeInTheDocument();
  });
});
