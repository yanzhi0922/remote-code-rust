import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { UserTextMessage } from './UserTextMessage';
import type { ConversationEntry } from '../../lib/types';

vi.mock('../chat/MarkdownRenderer', () => ({
  default: ({ content }: { content: string }) => <div>{content}</div>,
}));

const baseEntry: ConversationEntry = {
  role: 'user',
  text: '你好，请帮我检查代码',
  content_blocks: [],
  tool_calls: [],
  tool_call_id: null,
  name: null,
  is_error: false,
};

describe('UserTextMessage', () => {
  afterEach(cleanup);

  it('渲染用户文本消息', () => {
    render(<UserTextMessage entry={baseEntry} />);
    expect(screen.getByText('你好，请帮我检查代码')).toBeInTheDocument();
  });

  it('空文本返回 null', () => {
    const { container } = render(
      <UserTextMessage entry={{ ...baseEntry, text: '   ' }} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <UserTextMessage entry={baseEntry} className="custom-class" />,
    );
    expect(container.firstChild).toHaveClass('custom-class');
  });

  it('使用深色气泡样式', () => {
    const { container } = render(<UserTextMessage entry={baseEntry} />);
    const bubble = container.querySelector('.bg-\\[\\#17181a\\]');
    expect(bubble).toBeInTheDocument();
  });
});
