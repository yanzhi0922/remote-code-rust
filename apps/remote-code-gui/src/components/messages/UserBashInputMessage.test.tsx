import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserBashInputMessage } from './UserBashInputMessage';

describe('UserBashInputMessage', () => {
  afterEach(cleanup);

  it('渲染 Bash 命令输入', () => {
    render(<UserBashInputMessage command="npm run build" />);
    expect(screen.getByTestId('user-bash-input')).toBeInTheDocument();
    expect(screen.getByText('npm run build')).toBeInTheDocument();
  });

  it('显示 $ 前缀', () => {
    render(<UserBashInputMessage command="ls -la" />);
    const prefix = screen.getByText('$ ', { exact: false, trim: false });
    expect(prefix).toBeInTheDocument();
    expect(prefix.tagName).toBe('SPAN');
  });

  it('使用 monospace 字体', () => {
    render(<UserBashInputMessage command="echo hello" />);
    const code = screen.getByText('echo hello');
    expect(code.tagName).toBe('CODE');
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <UserBashInputMessage command="test" className="bash-custom" />,
    );
    expect(container.firstChild).toHaveClass('bash-custom');
  });

  it('空命令也能渲染', () => {
    render(<UserBashInputMessage command="" />);
    expect(screen.getByTestId('user-bash-input')).toBeInTheDocument();
  });
});
