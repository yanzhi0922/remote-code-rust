import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { MessageModel } from './MessageModel';

describe('MessageModel', () => {
  afterEach(cleanup);

  it('渲染模型名称', () => {
    render(<MessageModel modelName="gpt-4" />);
    expect(screen.getByTestId('message-model')).toBeInTheDocument();
    expect(screen.getByText('gpt-4')).toBeInTheDocument();
  });

  it('使用小标签样式', () => {
    const { container } = render(<MessageModel modelName="claude-3" />);
    expect(container.firstChild).toHaveClass('rounded-full');
    expect(container.firstChild).toHaveClass('bg-slate-100');
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <MessageModel modelName="test" className="model-custom" />,
    );
    expect(container.firstChild).toHaveClass('model-custom');
  });

  it('显示不同模型名称', () => {
    render(<MessageModel modelName="gemini-pro" />);
    expect(screen.getByText('gemini-pro')).toBeInTheDocument();
  });
});
