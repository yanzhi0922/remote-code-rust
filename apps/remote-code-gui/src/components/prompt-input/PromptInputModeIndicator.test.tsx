import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PromptInputModeIndicator } from './PromptInputModeIndicator';

describe('PromptInputModeIndicator', () => {
  afterEach(cleanup);

  it('prompt 模式下不渲染', () => {
    render(<PromptInputModeIndicator mode="prompt" />);
    expect(screen.queryByTestId('prompt-mode-indicator')).not.toBeInTheDocument();
  });

  it('bash 模式下显示 BASH 标签', () => {
    render(<PromptInputModeIndicator mode="bash" />);
    expect(screen.getByTestId('prompt-mode-indicator')).toBeInTheDocument();
    expect(screen.getByText('BASH')).toBeInTheDocument();
  });

  it('bash 模式下显示红色样式', () => {
    render(<PromptInputModeIndicator mode="bash" />);
    const indicator = screen.getByTestId('prompt-mode-indicator');
    expect(indicator.className).toContain('bg-red-100');
    expect(indicator.className).toContain('text-red-700');
  });

  it('vim-normal 模式下显示 NORMAL 标签', () => {
    render(<PromptInputModeIndicator mode="vim-normal" />);
    expect(screen.getByText('NORMAL')).toBeInTheDocument();
  });

  it('vim-insert 模式下显示 INSERT 标签', () => {
    render(<PromptInputModeIndicator mode="vim-insert" />);
    expect(screen.getByText('INSERT')).toBeInTheDocument();
  });

  it('vim-normal 使用琥珀色样式', () => {
    render(<PromptInputModeIndicator mode="vim-normal" />);
    const indicator = screen.getByTestId('prompt-mode-indicator');
    expect(indicator.className).toContain('bg-amber-100');
  });

  it('vim-insert 使用绿色样式', () => {
    render(<PromptInputModeIndicator mode="vim-insert" />);
    const indicator = screen.getByTestId('prompt-mode-indicator');
    expect(indicator.className).toContain('bg-green-100');
  });
});
