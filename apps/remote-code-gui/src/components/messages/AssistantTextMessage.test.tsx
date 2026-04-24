import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AssistantTextMessage } from './AssistantTextMessage';
import type { ConversationEntry } from '../../lib/types';

const baseEntry: ConversationEntry = {
  role: 'assistant',
  text: '这是助手的回复内容',
  content_blocks: [],
  tool_calls: [],
  tool_call_id: null,
  name: null,
  is_error: false,
};

describe('AssistantTextMessage', () => {
  afterEach(cleanup);

  it('渲染助手文本消息', () => {
    render(<AssistantTextMessage entry={baseEntry} />);
    expect(screen.getByTestId('assistant-text-message')).toBeInTheDocument();
    expect(screen.getByText('这是助手的回复内容')).toBeInTheDocument();
  });

  it('空文本返回 null', () => {
    const { container } = render(
      <AssistantTextMessage entry={{ ...baseEntry, text: '   ' }} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('检测 rate limit 错误并显示红色警告', () => {
    render(
      <AssistantTextMessage
        entry={{ ...baseEntry, text: 'Rate limit exceeded. Please try again.' }}
      />,
    );
    expect(screen.getByText('Rate Limit')).toBeInTheDocument();
  });

  it('检测 timeout 错误并显示红色警告', () => {
    render(
      <AssistantTextMessage
        entry={{ ...baseEntry, text: 'Request timed out after 30s' }}
      />,
    );
    expect(screen.getByText('Timeout')).toBeInTheDocument();
  });

  it('检测 API error 并显示红色警告', () => {
    render(
      <AssistantTextMessage
        entry={{ ...baseEntry, text: 'API error: Internal Server Error' }}
      />,
    );
    expect(screen.getByText('API Error')).toBeInTheDocument();
  });

  it('长文本显示展开/折叠按钮', () => {
    const longText = 'a'.repeat(700);
    render(<AssistantTextMessage entry={{ ...baseEntry, text: longText }} />);
    expect(screen.getByText('展开全部')).toBeInTheDocument();
  });

  it('点击展开按钮切换文本', () => {
    const longText = 'a'.repeat(700);
    render(<AssistantTextMessage entry={{ ...baseEntry, text: longText }} />);
    const btn = screen.getByText('展开全部');
    fireEvent.click(btn);
    expect(screen.getByText('收起')).toBeInTheDocument();
  });

  it('transcript 模式不显示折叠按钮', () => {
    const longText = 'a'.repeat(700);
    render(
      <AssistantTextMessage
        entry={{ ...baseEntry, text: longText }}
        isTranscriptMode
      />,
    );
    expect(screen.queryByText('展开全部')).not.toBeInTheDocument();
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <AssistantTextMessage entry={baseEntry} className="my-custom" />,
    );
    expect(container.firstChild).toHaveClass('my-custom');
  });
});
