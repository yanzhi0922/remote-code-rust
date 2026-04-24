import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ShutdownMessage } from './ShutdownMessage';

describe('ShutdownMessage', () => {
  afterEach(cleanup);

  it('渲染 request 变体', () => {
    render(<ShutdownMessage variant="request" from="alice" />);
    expect(screen.getByTestId('shutdown-message')).toBeInTheDocument();
    expect(screen.getByText('Shutdown request from alice')).toBeInTheDocument();
  });

  it('request 变体使用黄色边框', () => {
    const { container } = render(
      <ShutdownMessage variant="request" from="alice" />,
    );
    expect(container.firstChild).toHaveClass('border-amber-300');
  });

  it('渲染 rejected 变体', () => {
    render(<ShutdownMessage variant="rejected" from="bob" reason="任务未完成" />);
    expect(screen.getByText('Shutdown rejected by bob')).toBeInTheDocument();
    expect(screen.getByText('任务未完成')).toBeInTheDocument();
  });

  it('rejected 变体使用灰色边框', () => {
    const { container } = render(
      <ShutdownMessage variant="rejected" from="bob" />,
    );
    expect(container.firstChild).toHaveClass('border-slate-300');
  });

  it('渲染 approved 变体', () => {
    render(<ShutdownMessage variant="approved" from="charlie" />);
    expect(screen.getByText('Shutdown approved by charlie')).toBeInTheDocument();
  });

  it('approved 变体使用绿色边框', () => {
    const { container } = render(
      <ShutdownMessage variant="approved" from="charlie" />,
    );
    expect(container.firstChild).toHaveClass('border-emerald-300');
  });

  it('无 reason 时不显示原因', () => {
    render(<ShutdownMessage variant="request" from="alice" />);
    expect(screen.queryByText(/原因/)).not.toBeInTheDocument();
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <ShutdownMessage variant="request" from="alice" className="shutdown-cls" />,
    );
    expect(container.firstChild).toHaveClass('shutdown-cls');
  });
});
