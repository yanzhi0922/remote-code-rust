import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserCommandMessage } from './UserCommandMessage';

describe('UserCommandMessage', () => {
  afterEach(cleanup);

  it('渲染命令消息', () => {
    render(<UserCommandMessage command="help" />);
    expect(screen.getByTestId('user-command-message')).toBeInTheDocument();
    expect(screen.getByText('/help')).toBeInTheDocument();
  });

  it('显示命令参数', () => {
    render(<UserCommandMessage command="model" args="gpt-4" />);
    expect(screen.getByText('/model')).toBeInTheDocument();
    expect(screen.getByText('gpt-4')).toBeInTheDocument();
  });

  it('无参数时只显示命令', () => {
    render(<UserCommandMessage command="clear" />);
    expect(screen.getByText('/clear')).toBeInTheDocument();
  });

  it('使用紫色样式', () => {
    const { container } = render(<UserCommandMessage command="test" />);
    expect(container.firstChild).toHaveClass('bg-violet-100');
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <UserCommandMessage command="test" className="cmd-custom" />,
    );
    expect(container.firstChild).toHaveClass('cmd-custom');
  });
});
