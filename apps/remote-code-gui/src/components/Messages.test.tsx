import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Messages } from './Messages';
import type { ConversationEntry } from '../lib/types';

vi.mock('./chat/MarkdownRenderer', () => ({
  default: ({ content }: { content: string }) => <div>{content}</div>,
}));

const userEntry: ConversationEntry = {
  role: 'user',
  text: '你好',
  content_blocks: [],
  tool_calls: [],
  tool_call_id: null,
  name: null,
  is_error: false,
};

const assistantEntry: ConversationEntry = {
  role: 'assistant',
  text: '你好！有什么可以帮你的？',
  content_blocks: [],
  tool_calls: [],
  tool_call_id: null,
  name: null,
  is_error: false,
};

const toolEntry: ConversationEntry = {
  role: 'tool',
  text: '文件内容正常',
  content_blocks: [],
  tool_calls: [],
  tool_call_id: 'tc-1',
  name: 'Read',
  is_error: false,
};

describe('Messages', () => {
  beforeEach(() => {
    Object.defineProperty(Element.prototype, 'scrollIntoView', {
      configurable: true,
      value: vi.fn(),
    });
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it('空对话显示空状态', () => {
    render(<Messages conversation={[]} />);
    expect(screen.getByText('会话已创建')).toBeInTheDocument();
  });

  it('渲染用户消息', () => {
    render(<Messages conversation={[userEntry]} />);
    expect(screen.getByText('你好')).toBeInTheDocument();
  });

  it('渲染助手消息', async () => {
    render(<Messages conversation={[userEntry, assistantEntry]} />);
    await waitFor(() => {
      expect(screen.getByText('你好！有什么可以帮你的？')).toBeInTheDocument();
    });
  });

  it('渲染工具结果消息', () => {
    render(<Messages conversation={[toolEntry]} />);
    expect(screen.getByText('文件内容正常')).toBeInTheDocument();
  });

  it('sending 状态显示加载指示器', () => {
    render(<Messages conversation={[userEntry]} sending />);
    expect(screen.getByText('正在处理当前请求…')).toBeInTheDocument();
  });

  it('sendError 显示错误信息', () => {
    render(<Messages conversation={[userEntry]} sendError="发送失败" />);
    expect(screen.getByText('发送失败')).toBeInTheDocument();
  });

  it('渲染完整的对话流程', async () => {
    render(
      <Messages
        conversation={[userEntry, assistantEntry, toolEntry]}
      />,
    );
    expect(screen.getByText('你好')).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText('你好！有什么可以帮你的？')).toBeInTheDocument();
    });
    expect(screen.getByText('文件内容正常')).toBeInTheDocument();
  });
});
