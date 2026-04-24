import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MessageResponse } from './MessageResponse';
import type { ConversationEntry } from '../lib/types';

vi.mock('./chat/MarkdownRenderer', () => ({
  default: ({ content }: { content: string }) => <div>{content}</div>,
}));

const baseEntry: ConversationEntry = {
  role: 'assistant',
  text: '这是助手回复',
  content_blocks: [],
  tool_calls: [],
  tool_call_id: null,
  name: null,
  is_error: false,
};

describe('MessageResponse', () => {
  afterEach(cleanup);

  it('渲染助手文本回复', async () => {
    render(<MessageResponse entry={baseEntry} />);
    expect(screen.getByText('助手')).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText('这是助手回复')).toBeInTheDocument();
    });
  });

  it('渲染 thinking 块', () => {
    const entry: ConversationEntry = {
      ...baseEntry,
      content_blocks: [
        { type: 'thinking', thinking: '先分析问题，再制定方案。' },
      ],
    };
    render(<MessageResponse entry={entry} />);
    expect(screen.getByText('含思考过程')).toBeInTheDocument();
  });

  it('渲染工具调用', () => {
    const entry: ConversationEntry = {
      ...baseEntry,
      tool_calls: [
        { id: 'tc-1', name: 'Bash', input: { command: 'ls -la' } },
      ],
    };
    render(<MessageResponse entry={entry} />);
    expect(screen.getByText(/1 个工具调用/)).toBeInTheDocument();
    expect(screen.getByText('Bash')).toBeInTheDocument();
  });

  it('无文本无工具时显示加载状态', () => {
    const entry: ConversationEntry = {
      ...baseEntry,
      text: '',
    };
    render(<MessageResponse entry={entry} />);
    expect(screen.getByText('正在生成回复…')).toBeInTheDocument();
  });

  it('compact 模式使用更小的内边距', () => {
    const { container } = render(<MessageResponse entry={baseEntry} compact />);
    const card = container.firstChild;
    expect(card).toHaveClass('px-4');
    expect(card).toHaveClass('py-3');
  });
});
