import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PromptInputQueuedCommands } from './PromptInputQueuedCommands';

describe('PromptInputQueuedCommands', () => {
  afterEach(cleanup);

  it('commands 为空时返回 null', () => {
    render(<PromptInputQueuedCommands commands={[]} onRemove={vi.fn()} />);
    expect(screen.queryByTestId('prompt-queued-commands')).not.toBeInTheDocument();
  });

  it('有命令时渲染并显示 data-testid', () => {
    render(
      <PromptInputQueuedCommands commands={['cmd1']} onRemove={vi.fn()} />,
    );
    expect(screen.getByTestId('prompt-queued-commands')).toBeInTheDocument();
  });

  it('显示所有命令', () => {
    render(
      <PromptInputQueuedCommands
        commands={['npm test', 'npm build']}
        onRemove={vi.fn()}
      />,
    );
    expect(screen.getByText(/npm test/)).toBeInTheDocument();
    expect(screen.getByText(/npm build/)).toBeInTheDocument();
  });

  it('点击删除按钮触发 onRemove', () => {
    const onRemove = vi.fn();
    render(
      <PromptInputQueuedCommands
        commands={['cmd1', 'cmd2']}
        onRemove={onRemove}
      />,
    );
    const buttons = screen.getAllByLabelText(/移除命令/);
    fireEvent.click(buttons[1]);
    expect(onRemove).toHaveBeenCalledWith(1);
  });

  it('每条命令都有删除按钮', () => {
    render(
      <PromptInputQueuedCommands
        commands={['a', 'b', 'c']}
        onRemove={vi.fn()}
      />,
    );
    const buttons = screen.getAllByLabelText(/移除命令/);
    expect(buttons).toHaveLength(3);
  });
});
