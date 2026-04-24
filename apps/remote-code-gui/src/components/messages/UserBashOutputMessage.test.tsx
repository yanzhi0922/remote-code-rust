import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UserBashOutputMessage } from './UserBashOutputMessage';

describe('UserBashOutputMessage', () => {
  afterEach(cleanup);

  it('渲染 Bash 输出', () => {
    render(<UserBashOutputMessage output="Build successful" />);
    expect(screen.getByTestId('user-bash-output')).toBeInTheDocument();
    expect(screen.getByText('Build successful')).toBeInTheDocument();
  });

  it('stdout 使用默认样式', () => {
    render(<UserBashOutputMessage output="normal output" stream="stdout" />);
    const { container } = render(<UserBashOutputMessage output="normal output" stream="stdout" />);
    expect(container.querySelector('pre')).toHaveClass('text-slate-700');
  });

  it('stderr 使用红色样式', () => {
    const { container } = render(
      <UserBashOutputMessage output="error output" stream="stderr" />,
    );
    expect(container.firstChild).toHaveClass('bg-red-50');
    expect(container.querySelector('pre')).toHaveClass('text-red-700');
  });

  it('exitCode !== 0 时显示退出码', () => {
    render(<UserBashOutputMessage output="failed" exitCode={1} />);
    expect(screen.getByText('Exit code: 1')).toBeInTheDocument();
  });

  it('exitCode === 0 时不显示退出码', () => {
    render(<UserBashOutputMessage output="ok" exitCode={0} />);
    expect(screen.queryByText(/Exit code/)).not.toBeInTheDocument();
  });

  it('无 exitCode 时不显示退出码', () => {
    render(<UserBashOutputMessage output="ok" />);
    expect(screen.queryByText(/Exit code/)).not.toBeInTheDocument();
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <UserBashOutputMessage output="test" className="output-custom" />,
    );
    expect(container.firstChild).toHaveClass('output-custom');
  });
});
