import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { CompactBoundaryMessage } from './CompactBoundaryMessage';

describe('CompactBoundaryMessage', () => {
  afterEach(cleanup);

  it('渲染压缩边界分隔线', () => {
    render(<CompactBoundaryMessage />);
    expect(screen.getByTestId('compact-boundary')).toBeInTheDocument();
    expect(screen.getByText('✻ Conversation compacted')).toBeInTheDocument();
  });

  it('包含分隔线元素', () => {
    const { container } = render(<CompactBoundaryMessage />);
    const dividers = container.querySelectorAll('.h-px');
    expect(dividers.length).toBe(2);
  });

  it('使用灰色文本样式', () => {
    const { container } = render(<CompactBoundaryMessage />);
    expect(container.firstChild).toHaveClass('text-slate-400');
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <CompactBoundaryMessage className="boundary-custom" />,
    );
    expect(container.firstChild).toHaveClass('boundary-custom');
  });
});
