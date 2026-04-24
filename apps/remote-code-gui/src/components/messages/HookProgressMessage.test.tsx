import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { HookProgressMessage } from './HookProgressMessage';

describe('HookProgressMessage', () => {
  afterEach(cleanup);

  it('渲染 Hook 进度消息', () => {
    render(
      <HookProgressMessage
        hookEvent="PreToolUse"
        inProgressCount={3}
        resolvedCount={2}
      />,
    );
    expect(screen.getByTestId('hook-progress-message')).toBeInTheDocument();
    expect(screen.getByText('5 PreToolUse hooks ran')).toBeInTheDocument();
  });

  it('inProgressCount 为 0 时返回 null', () => {
    const { container } = render(
      <HookProgressMessage
        hookEvent="PreToolUse"
        inProgressCount={0}
        resolvedCount={5}
      />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('显示总数（inProgress + resolved）', () => {
    render(
      <HookProgressMessage
        hookEvent="PostToolUse"
        inProgressCount={2}
        resolvedCount={3}
      />,
    );
    expect(screen.getByText('5 PostToolUse hooks ran')).toBeInTheDocument();
  });

  it('只有 inProgressCount 时显示正确', () => {
    render(
      <HookProgressMessage
        hookEvent="PreToolUse"
        inProgressCount={1}
        resolvedCount={0}
      />,
    );
    expect(screen.getByText('1 PreToolUse hooks ran')).toBeInTheDocument();
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <HookProgressMessage
        hookEvent="PreToolUse"
        inProgressCount={1}
        resolvedCount={0}
        className="hook-custom"
      />,
    );
    expect(container.firstChild).toHaveClass('hook-custom');
  });
});
