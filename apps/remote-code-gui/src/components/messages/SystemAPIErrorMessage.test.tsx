import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { SystemAPIErrorMessage } from './SystemAPIErrorMessage';

describe('SystemAPIErrorMessage', () => {
  afterEach(cleanup);

  it('渲染 API 错误消息', () => {
    render(<SystemAPIErrorMessage message="Internal Server Error" />);
    expect(screen.getByTestId('system-api-error')).toBeInTheDocument();
    expect(screen.getByText('Internal Server Error')).toBeInTheDocument();
    expect(screen.getByText('API Error')).toBeInTheDocument();
  });

  it('显示状态码标签', () => {
    render(<SystemAPIErrorMessage message="Not Found" statusCode={404} />);
    expect(screen.getByText('404')).toBeInTheDocument();
  });

  it('无状态码时不显示标签', () => {
    render(<SystemAPIErrorMessage message="Error" />);
    expect(screen.queryByText(/^\d+$/)).not.toBeInTheDocument();
  });

  it('长消息显示展开按钮', () => {
    const longMsg = 'x'.repeat(600);
    render(<SystemAPIErrorMessage message={longMsg} />);
    expect(screen.getByText('展开全部')).toBeInTheDocument();
  });

  it('点击展开显示完整消息', () => {
    const longMsg = 'x'.repeat(600);
    render(<SystemAPIErrorMessage message={longMsg} />);
    fireEvent.click(screen.getByText('展开全部'));
    expect(screen.getByText('收起')).toBeInTheDocument();
  });

  it('短消息不显示展开按钮', () => {
    render(<SystemAPIErrorMessage message="Short error" />);
    expect(screen.queryByText('展开全部')).not.toBeInTheDocument();
  });

  it('使用红色边框样式', () => {
    const { container } = render(<SystemAPIErrorMessage message="err" />);
    expect(container.firstChild).toHaveClass('border-red-300');
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <SystemAPIErrorMessage message="err" className="api-err" />,
    );
    expect(container.firstChild).toHaveClass('api-err');
  });
});
