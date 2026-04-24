import { cleanup, render, screen } from '@testing-library/react';
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
});
