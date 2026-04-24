import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { GroupedToolUseContent } from './GroupedToolUseContent';
import type { ToolCallInfo } from '../../lib/types';

const toolCalls: ToolCallInfo[] = [
  { id: 'tc-1', name: 'Read', input: { file_path: '/src/index.ts' } },
  { id: 'tc-2', name: 'Bash', input: { command: 'npm test' } },
  { id: 'tc-3', name: 'Write', input: { file_path: '/src/output.ts', content: 'hello' } },
];

describe('GroupedToolUseContent', () => {
  afterEach(cleanup);

  it('渲染分组工具使用', () => {
    render(<GroupedToolUseContent toolCalls={toolCalls} />);
    expect(screen.getByTestId('grouped-tool-use')).toBeInTheDocument();
    expect(screen.getByText('3 tools called')).toBeInTheDocument();
  });

  it('显示每个工具名称', () => {
    render(<GroupedToolUseContent toolCalls={toolCalls} />);
    expect(screen.getByText('Read')).toBeInTheDocument();
    expect(screen.getByText('Bash')).toBeInTheDocument();
    expect(screen.getByText('Write')).toBeInTheDocument();
  });

  it('单个工具显示单数形式', () => {
    render(
      <GroupedToolUseContent toolCalls={[toolCalls[0]]} />,
    );
    expect(screen.getByText('1 tool called')).toBeInTheDocument();
  });

  it('空列表返回 null', () => {
    const { container } = render(<GroupedToolUseContent toolCalls={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <GroupedToolUseContent toolCalls={toolCalls} className="grouped-custom" />,
    );
    expect(container.firstChild).toHaveClass('grouped-custom');
  });

  it('截断长输入', () => {
    const longInput = { data: 'x'.repeat(100) };
    render(
      <GroupedToolUseContent
        toolCalls={[{ id: 'tc-long', name: 'Tool', input: longInput }]}
      />,
    );
    expect(screen.getByText('Tool')).toBeInTheDocument();
  });
});
