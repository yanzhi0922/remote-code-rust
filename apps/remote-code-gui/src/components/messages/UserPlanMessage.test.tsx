import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserPlanMessage } from './UserPlanMessage';

describe('UserPlanMessage', () => {
  afterEach(cleanup);

  it('渲染计划内容', () => {
    render(<UserPlanMessage planContent="1. 重构模块 A\n2. 添加测试" />);
    expect(screen.getByText('用户计划')).toBeInTheDocument();
    expect(screen.getByText(/重构模块 A/)).toBeInTheDocument();
  });

  it('空内容返回 null', () => {
    const { container } = render(<UserPlanMessage planContent="   " />);
    expect(container.innerHTML).toBe('');
  });

  it('默认添加外边距', () => {
    const { container } = render(<UserPlanMessage planContent="计划" />);
    expect(container.firstChild).toHaveClass('my-2');
  });

  it('addMargin=false 时不添加外边距', () => {
    const { container } = render(
      <UserPlanMessage planContent="计划" addMargin={false} />,
    );
    expect(container.firstChild).not.toHaveClass('my-2');
  });

  it('支持自定义 className', () => {
    const { container } = render(
      <UserPlanMessage planContent="计划" className="extra" />,
    );
    expect(container.firstChild).toHaveClass('extra');
  });
});
