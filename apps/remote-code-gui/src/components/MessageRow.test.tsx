import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { MessageRow } from './MessageRow';

describe('MessageRow', () => {
  afterEach(cleanup);

  it('system 角色返回 null', () => {
    const { container } = render(
      <MessageRow role="system">内容</MessageRow>,
    );
    expect(container.innerHTML).toBe('');
  });

  it('渲染助手消息行', () => {
    render(
      <MessageRow role="assistant" timestamp="2026-04-24T10:00:00.000Z">
        <div>助手回复</div>
      </MessageRow>,
    );
    expect(screen.getByText('助手')).toBeInTheDocument();
    expect(screen.getByText('助手回复')).toBeInTheDocument();
  });

  it('渲染用户消息行', () => {
    render(
      <MessageRow role="user" timestamp="2026-04-24T10:00:00.000Z">
        <div>用户消息</div>
      </MessageRow>,
    );
    expect(screen.getByText('用户')).toBeInTheDocument();
    expect(screen.getByText('用户消息')).toBeInTheDocument();
  });

  it('渲染工具消息行', () => {
    render(
      <MessageRow role="tool" timestamp="2026-04-24T10:00:00.000Z">
        <div>工具结果</div>
      </MessageRow>,
    );
    expect(screen.getByText('工具')).toBeInTheDocument();
  });

  it('用户连续消息不显示头像', () => {
    const { container } = render(
      <MessageRow role="user" isUserContinuation>
        <div>连续消息</div>
      </MessageRow>,
    );
    // 不应有头像圆形容器
    expect(container.querySelector('.rounded-full')).not.toBeInTheDocument();
  });

  it('显示操作按钮', () => {
    render(
      <MessageRow
        role="user"
        messageText="测试"
        messageId="1"
        showActions
      >
        <div>内容</div>
      </MessageRow>,
    );
    expect(screen.getByTitle('复制')).toBeInTheDocument();
  });

  it('showActions=false 不显示操作按钮', () => {
    render(
      <MessageRow role="user" showActions={false}>
        <div>内容</div>
      </MessageRow>,
    );
    expect(screen.queryByTitle('复制')).not.toBeInTheDocument();
  });
});
