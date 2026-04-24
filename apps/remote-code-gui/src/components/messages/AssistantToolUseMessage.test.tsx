import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AssistantToolUseMessage } from './AssistantToolUseMessage';
import type { ToolCallInfo } from '../../lib/types';

const baseToolCall: ToolCallInfo = {
  id: 'tc-1',
  name: 'Read',
  input: { file_path: '/src/index.ts' },
};

describe('AssistantToolUseMessage', () => {
  afterEach(cleanup);

  it('运行中显示 spinner 和工具名', () => {
    render(
      <AssistantToolUseMessage
        toolCall={baseToolCall}
        isRunning
        isResolved={false}
        isError={false}
      />,
    );
    expect(screen.getByTestId('assistant-tool-use-message')).toBeInTheDocument();
    expect(screen.getByText('Read')).toBeInTheDocument();
  });

  it('已完成显示绿色样式', () => {
    const { container } = render(
      <AssistantToolUseMessage
        toolCall={baseToolCall}
        isRunning={false}
        isResolved
        isError={false}
      />,
    );
    expect(container.firstChild).toHaveClass('border-emerald-200');
  });

  it('错误显示红色样式', () => {
    const { container } = render(
      <AssistantToolUseMessage
        toolCall={baseToolCall}
        isRunning={false}
        isResolved
        isError
      />,
    );
    expect(container.firstChild).toHaveClass('border-red-200');
  });

  it('显示进度消息', () => {
    render(
      <AssistantToolUseMessage
        toolCall={baseToolCall}
        isRunning
        isResolved={false}
        isError={false}
        progress={{ tool_call_id: 'tc-1', tool_name: 'Read', message: '读取中...' }}
      />,
    );
    expect(screen.getByText('读取中...')).toBeInTheDocument();
  });

  it('verbose 模式显示详情按钮', () => {
    render(
      <AssistantToolUseMessage
        toolCall={baseToolCall}
        isRunning={false}
        isResolved
        isError={false}
        verbose
      />,
    );
    expect(screen.getByText('详情')).toBeInTheDocument();
  });

  it('verbose 模式点击展开显示 JSON 输入', () => {
    render(
      <AssistantToolUseMessage
        toolCall={baseToolCall}
        isRunning={false}
        isResolved
        isError={false}
        verbose
      />,
    );
    fireEvent.click(screen.getByText('详情'));
    expect(screen.getByText(/file_path/)).toBeInTheDocument();
  });

  it('字符串输入直接显示', () => {
    render(
      <AssistantToolUseMessage
        toolCall={{ id: 'tc-2', name: 'Bash', input: 'ls -la' }}
        isRunning={false}
        isResolved
        isError={false}
      />,
    );
    expect(screen.getByText('ls -la')).toBeInTheDocument();
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <AssistantToolUseMessage
        toolCall={baseToolCall}
        isRunning
        isResolved={false}
        isError={false}
        className="my-tool"
      />,
    );
    expect(container.firstChild).toHaveClass('my-tool');
  });
});
