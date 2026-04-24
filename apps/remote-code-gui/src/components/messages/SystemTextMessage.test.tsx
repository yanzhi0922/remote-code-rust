import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { SystemTextMessage } from './SystemTextMessage';
import type { ConversationEntry } from '../../lib/types';

const baseEntry: ConversationEntry = {
  role: 'system',
  text: 'System message',
  content_blocks: [],
  tool_calls: [],
  tool_call_id: null,
  name: null,
  is_error: false,
};

describe('SystemTextMessage', () => {
  afterEach(cleanup);

  it('渲染默认系统消息', () => {
    render(<SystemTextMessage entry={baseEntry} />);
    expect(screen.getByTestId('system-text-message')).toBeInTheDocument();
    expect(screen.getByText('System message')).toBeInTheDocument();
  });

  it('compacted 消息使用灰色样式', () => {
    const { container } = render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Context compacted successfully' }}
      />,
    );
    expect(container.firstChild).toHaveClass('border-slate-300');
  });

  it('error 消息使用红色样式', () => {
    const { container } = render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'An error occurred during processing' }}
      />,
    );
    expect(container.firstChild).toHaveClass('border-red-200');
  });

  it('warning 消息使用黄色样式', () => {
    const { container } = render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Warning: context window is nearly full' }}
      />,
    );
    expect(container.firstChild).toHaveClass('border-amber-200');
  });

  it('默认消息使用灰色系统样式', () => {
    const { container } = render(
      <SystemTextMessage entry={baseEntry} />,
    );
    expect(container.firstChild).toHaveClass('border-slate-200');
  });

  it('verbose 模式显示完整文本', () => {
    const longText = 'a'.repeat(300);
    render(<SystemTextMessage entry={{ ...baseEntry, text: longText }} verbose />);
    expect(screen.getByText(longText)).toBeInTheDocument();
  });

  it('非 verbose 模式截断长文本', () => {
    const longText = 'a'.repeat(300);
    render(<SystemTextMessage entry={{ ...baseEntry, text: longText }} />);
    expect(screen.getByText(longText.slice(0, 200))).toBeInTheDocument();
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <SystemTextMessage entry={baseEntry} className="sys-custom" />,
    );
    expect(container.firstChild).toHaveClass('sys-custom');
  });

  // ── TurnDurationMessage ──

  it('检测 turn duration 消息', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Turn completed in 12.5s' }}
      />,
    );
    expect(screen.getByTestId('turn-duration-message')).toBeInTheDocument();
    expect(screen.getByText('本轮对话耗时 12.5s')).toBeInTheDocument();
  });

  it('检测中文 turn duration 消息', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: '本轮耗时 8.3秒' }}
      />,
    );
    expect(screen.getByTestId('turn-duration-message')).toBeInTheDocument();
  });

  it('turn duration 使用蓝色样式', () => {
    const { container } = render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Turn completed in 5.0s' }}
      />,
    );
    expect(container.firstChild).toHaveClass('border-blue-200');
  });

  // ── MemorySavedMessage ──

  it('检测 memory saved 消息', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Memory saved to memory.md' }}
      />,
    );
    expect(screen.getByTestId('memory-saved-message')).toBeInTheDocument();
    expect(screen.getByText('记忆已保存')).toBeInTheDocument();
  });

  it('memory saved 使用绿色样式', () => {
    const { container } = render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Memory saved successfully' }}
      />,
    );
    expect(container.firstChild).toHaveClass('border-green-200');
  });

  it('memory saved 提取文件路径链接', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Saved to memory file memory.md and notes.ts' }}
      />,
    );
    // Multiple file paths are extracted, use getAllByTestId
    const links = screen.getAllByTestId('file-path-link');
    expect(links.length).toBeGreaterThanOrEqual(1);
  });

  // ── ThinkingMessage ──

  it('检测 thinking 消息', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Thinking...' }}
      />,
    );
    expect(screen.getByTestId('thinking-message')).toBeInTheDocument();
    expect(screen.getByText('思考中...')).toBeInTheDocument();
  });

  it('thinking 使用紫色样式', () => {
    const { container } = render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Thinking...' }}
      />,
    );
    expect(container.firstChild).toHaveClass('border-purple-200');
  });

  // ── BridgeStatusMessage ──

  it('检测 bridge connected 消息', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Bridge connected successfully' }}
      />,
    );
    expect(screen.getByTestId('bridge-status-message')).toBeInTheDocument();
    expect(screen.getByText('Bridge 已连接')).toBeInTheDocument();
  });

  it('检测 bridge disconnected 消息', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Bridge disconnected' }}
      />,
    );
    expect(screen.getByTestId('bridge-status-message')).toBeInTheDocument();
    expect(screen.getByText('Bridge 已断开')).toBeInTheDocument();
  });

  it('bridge connected 使用绿色样式', () => {
    const { container } = render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'IDE connected' }}
      />,
    );
    expect(container.firstChild).toHaveClass('border-emerald-200');
  });

  it('bridge disconnected 使用橙色样式', () => {
    const { container } = render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'IDE disconnected' }}
      />,
    );
    expect(container.firstChild).toHaveClass('border-orange-200');
  });

  // ── StopHookSummary ──

  it('检测 stop hook summary 消息', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Stop hook executed: pre-commit check passed' }}
      />,
    );
    expect(screen.getByTestId('stop-hook-summary-message')).toBeInTheDocument();
    expect(screen.getByText('Stop Hook 摘要')).toBeInTheDocument();
  });

  // ── CompactBoundary ──

  it('检测 compact boundary 消息', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Compact boundary marker' }}
      />,
    );
    expect(screen.getByTestId('compact-boundary-message')).toBeInTheDocument();
    expect(screen.getByText('— 压缩边界 —')).toBeInTheDocument();
  });

  // ── PermissionRetry ──

  it('检测 permission retry 消息', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Permission retry: retrying with elevated access' }}
      />,
    );
    expect(screen.getByTestId('permission-retry-message')).toBeInTheDocument();
  });

  // ── ScheduledTask ──

  it('检测 scheduled task 消息', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: 'Scheduled task fired: daily cleanup cron' }}
      />,
    );
    expect(screen.getByTestId('scheduled-task-message')).toBeInTheDocument();
  });

  // ── 可折叠内容 ──

  it('长文本显示展开按钮', () => {
    const longText = 'a'.repeat(300);
    render(<SystemTextMessage entry={{ ...baseEntry, text: longText }} />);
    expect(screen.getByTestId('system-message-toggle')).toBeInTheDocument();
  });

  it('点击展开按钮显示全文', () => {
    const longText = 'a'.repeat(300);
    render(<SystemTextMessage entry={{ ...baseEntry, text: longText }} />);
    fireEvent.click(screen.getByTestId('system-message-toggle'));
    expect(screen.getByText(longText)).toBeInTheDocument();
  });

  it('点击收起按钮截断文本', () => {
    const longText = 'a'.repeat(300);
    render(<SystemTextMessage entry={{ ...baseEntry, text: longText }} />);
    // 展开
    fireEvent.click(screen.getByTestId('system-message-toggle'));
    // 收起
    fireEvent.click(screen.getByTestId('system-message-toggle'));
    expect(screen.getByText(longText.slice(0, 200))).toBeInTheDocument();
  });

  // ── 边界情况 ──

  it('空文本不崩溃', () => {
    render(<SystemTextMessage entry={{ ...baseEntry, text: '' }} />);
    expect(screen.getByTestId('system-text-message')).toBeInTheDocument();
  });

  it('特殊字符不崩溃', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: '<script>alert("xss")</script>' }}
      />,
    );
    expect(screen.getByTestId('system-text-message')).toBeInTheDocument();
  });

  it('超长文本不崩溃', () => {
    const veryLongText = 'x'.repeat(10000);
    render(<SystemTextMessage entry={{ ...baseEntry, text: veryLongText }} />);
    expect(screen.getByTestId('system-text-message')).toBeInTheDocument();
  });

  it('中文文本正确检测', () => {
    render(
      <SystemTextMessage
        entry={{ ...baseEntry, text: '记忆已保存到文件' }}
      />,
    );
    expect(screen.getByTestId('memory-saved-message')).toBeInTheDocument();
  });
});
