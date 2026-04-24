import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AssistantThinkingMessage } from './AssistantThinkingMessage';

describe('AssistantThinkingMessage', () => {
  afterEach(cleanup);

  it('渲染思考消息折叠行', () => {
    render(<AssistantThinkingMessage thinking="让我分析一下这个问题" />);
    expect(screen.getByTestId('assistant-thinking-message')).toBeInTheDocument();
    expect(screen.getByText('∴ Thinking')).toBeInTheDocument();
  });

  it('空内容返回 null', () => {
    const { container } = render(<AssistantThinkingMessage thinking="   " />);
    expect(container.innerHTML).toBe('');
  });

  it('默认模式不显示思考内容', () => {
    render(<AssistantThinkingMessage thinking="深度思考内容" />);
    expect(screen.queryByText('深度思考内容')).not.toBeInTheDocument();
  });

  it('verbose 模式直接显示思考内容', () => {
    render(<AssistantThinkingMessage thinking="深度思考内容" verbose />);
    expect(screen.getByText('深度思考内容')).toBeInTheDocument();
  });

  it('transcript 模式直接显示思考内容', () => {
    render(<AssistantThinkingMessage thinking="深度思考内容" isTranscriptMode />);
    expect(screen.getByText('深度思考内容')).toBeInTheDocument();
  });

  it('点击展开按钮显示思考内容', () => {
    render(<AssistantThinkingMessage thinking="展开后的思考内容" />);
    const btn = screen.getByText('∴ Thinking');
    fireEvent.click(btn);
    expect(screen.getByText('展开后的思考内容')).toBeInTheDocument();
  });

  it('再次点击折叠思考内容', () => {
    render(<AssistantThinkingMessage thinking="可折叠内容" />);
    const btn = screen.getByText('∴ Thinking');
    fireEvent.click(btn);
    expect(screen.getByText('可折叠内容')).toBeInTheDocument();
    fireEvent.click(btn);
    expect(screen.queryByText('可折叠内容')).not.toBeInTheDocument();
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <AssistantThinkingMessage thinking="test" className="custom-cls" />,
    );
    expect(container.firstChild).toHaveClass('custom-cls');
  });
});
