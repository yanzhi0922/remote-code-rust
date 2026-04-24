import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { MessageTimestamp } from './MessageTimestamp';

describe('MessageTimestamp', () => {
  afterEach(cleanup);

  it('渲染格式化时间', () => {
    render(
      <MessageTimestamp timestamp="2026-04-24T10:30:00.000Z" isTranscriptMode />,
    );
    // zh-CN 格式化，时区 UTC+8 → 18:30
    expect(screen.getByText(/18:30/)).toBeInTheDocument();
  });

  it('null 时间戳不渲染', () => {
    const { container } = render(<MessageTimestamp timestamp={null} />);
    expect(container.innerHTML).toBe('');
  });

  it('transcript 模式下始终可见', () => {
    const { container } = render(
      <MessageTimestamp timestamp="2026-04-24T10:30:00.000Z" isTranscriptMode />,
    );
    const span = container.querySelector('span');
    expect(span).not.toHaveClass('opacity-0');
  });

  it('非 transcript 模式下默认隐藏', () => {
    const { container } = render(
      <MessageTimestamp timestamp="2026-04-24T10:30:00.000Z" />,
    );
    const span = container.querySelector('span');
    expect(span).toHaveClass('opacity-0');
  });

  it('支持自定义 className', () => {
    const { container } = render(
      <MessageTimestamp
        timestamp="2026-04-24T10:30:00.000Z"
        isTranscriptMode
        className="custom"
      />,
    );
    const span = container.querySelector('span');
    expect(span).toHaveClass('custom');
  });
});
