import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PromptInputStashNotice } from './PromptInputStashNotice';

describe('PromptInputStashNotice', () => {
  afterEach(cleanup);

  it('hasStashedInput=false 时返回 null', () => {
    render(
      <PromptInputStashNotice hasStashedInput={false} onRestore={vi.fn()} />,
    );
    expect(screen.queryByTestId('prompt-stash-notice')).not.toBeInTheDocument();
  });

  it('hasStashedInput=true 时渲染并显示 data-testid', () => {
    render(
      <PromptInputStashNotice hasStashedInput onRestore={vi.fn()} />,
    );
    expect(screen.getByTestId('prompt-stash-notice')).toBeInTheDocument();
  });

  it('显示暂存提示文本', () => {
    render(
      <PromptInputStashNotice hasStashedInput onRestore={vi.fn()} />,
    );
    expect(
      screen.getByText(/You have stashed input/),
    ).toBeInTheDocument();
  });

  it('显示 Ctrl+Shift+U 提示', () => {
    render(
      <PromptInputStashNotice hasStashedInput onRestore={vi.fn()} />,
    );
    expect(
      screen.getByText(/Ctrl\+Shift\+U/),
    ).toBeInTheDocument();
  });

  it('点击 Restore 按钮触发 onRestore', () => {
    const onRestore = vi.fn();
    render(
      <PromptInputStashNotice hasStashedInput onRestore={onRestore} />,
    );
    fireEvent.click(screen.getByText('Restore'));
    expect(onRestore).toHaveBeenCalled();
  });
});
