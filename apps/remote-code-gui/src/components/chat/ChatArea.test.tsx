import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ChatArea } from './ChatArea';
import { resetAppStore } from '../../test/appStoreTestUtils';

vi.mock('./MarkdownRenderer', () => ({
  default: ({ content }: { content: string }) => <div>{content}</div>,
}));

describe('ChatArea', () => {
  beforeEach(() => {
    resetAppStore();
    Object.defineProperty(Element.prototype, 'scrollIntoView', {
      configurable: true,
      value: vi.fn(),
    });
  });

  afterEach(() => {
    cleanup();
    resetAppStore();
    vi.clearAllMocks();
  });

  it('shows the empty state before a session is selected', () => {
    render(<ChatArea />);

    expect(screen.getByText('Workbench')).toBeInTheDocument();
    expect(screen.getByText('Projects')).toBeInTheDocument();
    expect(screen.getByText('Recent Sessions')).toBeInTheDocument();
    expect(screen.getByText('Active Project')).toBeInTheDocument();
  });

  it('renders conversation cards, tool details, live progress, and send errors', async () => {
    resetAppStore({
      activeSessionId: 'session-1',
      conversation: [
        {
          role: 'user',
          text: '检查最新日志',
          content_blocks: [],
          tool_calls: [],
          tool_call_id: null,
          name: null,
          is_error: false,
        },
        {
          role: 'assistant',
          text: '已经完成排查。',
          content_blocks: [{ type: 'thinking', thinking: '先检查错误聚合，再确认修复入口。' }],
          tool_calls: [
            {
              id: 'tool-1',
              name: 'shell_command',
              input: { command: 'rg ERROR logs/app.log' },
            },
          ],
          tool_call_id: null,
          name: null,
          is_error: false,
        },
        {
          role: 'tool',
          text: 'found 3 matching lines',
          content_blocks: [],
          tool_calls: [],
          tool_call_id: 'tool-1',
          name: 'shell_command',
          is_error: false,
        },
      ],
      sending: true,
      sendError: '网络波动，请稍后重试',
      liveToolProgress: [
        {
          tool_call_id: 'tool-1',
          tool_name: 'shell_command',
          message: '继续收集错误上下文',
        },
      ],
      liveToolResults: [
        {
          tool_call_id: 'tool-1',
          tool_name: 'shell_command',
          is_error: false,
          output: '日志扫描完成',
        },
      ],
    });

    render(<ChatArea />);

    expect(screen.getByText('检查最新日志')).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Conversation transcript' })).toBeInTheDocument();
    expect(await screen.findByText('已经完成排查。')).toBeInTheDocument();
    expect(screen.getByText('Thinking')).toBeInTheDocument();
    expect(screen.getAllByText('shell_command').length).toBeGreaterThan(0);
    expect(screen.getByText('正在处理当前请求…')).toBeInTheDocument();
    expect(screen.getByText('继续收集错误上下文')).toBeInTheDocument();
    expect(screen.getByText('日志扫描完成')).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText('网络波动，请稍后重试')).toBeInTheDocument();
    });
  });
});
