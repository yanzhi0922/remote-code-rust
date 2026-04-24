import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { RateLimitMessage } from './RateLimitMessage';

describe('RateLimitMessage', () => {
  afterEach(cleanup);

  it('渲染速率限制消息', () => {
    render(<RateLimitMessage text="请求过于频繁，请稍后再试" />);
    expect(screen.getByTestId('rate-limit-message')).toBeInTheDocument();
    expect(screen.getByText('请求过于频繁，请稍后再试')).toBeInTheDocument();
  });

  it('使用黄色边框样式', () => {
    const { container } = render(<RateLimitMessage text="Rate limited" />);
    expect(container.firstChild).toHaveClass('border-amber-300');
  });

  it('有 onUpgrade 时显示升级按钮', () => {
    const onUpgrade = vi.fn();
    render(<RateLimitMessage text="Rate limited" onUpgrade={onUpgrade} />);
    expect(screen.getByText('Upgrade')).toBeInTheDocument();
  });

  it('点击升级按钮触发回调', () => {
    const onUpgrade = vi.fn();
    render(<RateLimitMessage text="Rate limited" onUpgrade={onUpgrade} />);
    fireEvent.click(screen.getByText('Upgrade'));
    expect(onUpgrade).toHaveBeenCalledTimes(1);
  });

  it('无 onUpgrade 时不显示升级按钮', () => {
    render(<RateLimitMessage text="Rate limited" />);
    expect(screen.queryByText('Upgrade')).not.toBeInTheDocument();
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <RateLimitMessage text="Rate limited" className="rate-custom" />,
    );
    expect(container.firstChild).toHaveClass('rate-custom');
  });
});
