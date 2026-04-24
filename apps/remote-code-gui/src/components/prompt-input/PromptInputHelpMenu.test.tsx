import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PromptInputHelpMenu } from './PromptInputHelpMenu';

describe('PromptInputHelpMenu', () => {
  afterEach(cleanup);

  it('visible=true 时渲染并显示 data-testid', () => {
    render(<PromptInputHelpMenu visible onClose={vi.fn()} />);
    expect(screen.getByTestId('prompt-help-menu')).toBeInTheDocument();
  });

  it('visible=false 时返回 null', () => {
    render(<PromptInputHelpMenu visible={false} onClose={vi.fn()} />);
    expect(screen.queryByTestId('prompt-help-menu')).not.toBeInTheDocument();
  });

  it('显示所有快捷键', () => {
    render(<PromptInputHelpMenu visible onClose={vi.fn()} />);
    expect(screen.getByText('发送消息')).toBeInTheDocument();
    expect(screen.getByText('换行')).toBeInTheDocument();
    expect(screen.getByText('Bash 模式')).toBeInTheDocument();
    expect(screen.getByText('斜杠命令')).toBeInTheDocument();
    expect(screen.getByText('取消')).toBeInTheDocument();
    expect(screen.getByText('清屏')).toBeInTheDocument();
  });

  it('显示快捷键标题', () => {
    render(<PromptInputHelpMenu visible onClose={vi.fn()} />);
    expect(screen.getByText('快捷键')).toBeInTheDocument();
  });

  it('点击关闭按钮触发 onClose', () => {
    const onClose = vi.fn();
    render(<PromptInputHelpMenu visible onClose={onClose} />);
    fireEvent.click(screen.getByLabelText('关闭'));
    expect(onClose).toHaveBeenCalled();
  });

  it('点击背景遮罩触发 onClose', () => {
    const onClose = vi.fn();
    render(<PromptInputHelpMenu visible onClose={onClose} />);
    const overlay = screen.getByTestId('prompt-help-menu');
    fireEvent.click(overlay);
    expect(onClose).toHaveBeenCalled();
  });

  it('显示快捷键键名', () => {
    render(<PromptInputHelpMenu visible onClose={vi.fn()} />);
    expect(screen.getByText('Enter')).toBeInTheDocument();
    expect(screen.getByText('Shift+Enter')).toBeInTheDocument();
    expect(screen.getByText('!command')).toBeInTheDocument();
  });
});
