import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AssistantRedactedThinkingMessage } from './AssistantRedactedThinkingMessage';

describe('AssistantRedactedThinkingMessage', () => {
  afterEach(cleanup);

  it('渲染占位符文本', () => {
    render(<AssistantRedactedThinkingMessage />);
    expect(screen.getByTestId('assistant-redacted-thinking')).toBeInTheDocument();
    expect(screen.getByText('Thinking redacted')).toBeInTheDocument();
  });

  it('使用灰色斜体样式', () => {
    const { container } = render(<AssistantRedactedThinkingMessage />);
    expect(container.firstChild).toHaveClass('italic');
    expect(container.firstChild).toHaveClass('text-slate-400');
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <AssistantRedactedThinkingMessage className="extra-class" />,
    );
    expect(container.firstChild).toHaveClass('extra-class');
  });

  it('data-testid 正确', () => {
    render(<AssistantRedactedThinkingMessage />);
    expect(screen.getByTestId('assistant-redacted-thinking')).toBeTruthy();
  });
});
