import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PromptInput } from './PromptInput';

describe('PromptInput', () => {
  afterEach(cleanup);

  it('渲染 textarea 并显示 data-testid', () => {
    render(
      <PromptInput value="" onChange={vi.fn()} onSubmit={vi.fn()} />,
    );
    expect(screen.getByTestId('prompt-input')).toBeInTheDocument();
  });

  it('显示传入的 value', () => {
    render(
      <PromptInput value="hello" onChange={vi.fn()} onSubmit={vi.fn()} />,
    );
    const textarea = screen.getByDisplayValue('hello');
    expect(textarea).toBeInTheDocument();
  });

  it('输入变化时调用 onChange', () => {
    const onChange = vi.fn();
    render(<PromptInput value="" onChange={onChange} onSubmit={vi.fn()} />);
    const textarea = screen.getByRole('textbox');
    fireEvent.change(textarea, { target: { value: 'new text' } });
    expect(onChange).toHaveBeenCalledWith('new text');
  });

  it('Enter 键触发 onSubmit', () => {
    const onSubmit = vi.fn();
    render(
      <PromptInput value="test" onChange={vi.fn()} onSubmit={onSubmit} />,
    );
    const textarea = screen.getByRole('textbox');
    fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: false });
    expect(onSubmit).toHaveBeenCalledWith('test');
  });

  it('Shift+Enter 不触发 onSubmit', () => {
    const onSubmit = vi.fn();
    render(
      <PromptInput value="test" onChange={vi.fn()} onSubmit={onSubmit} />,
    );
    const textarea = screen.getByRole('textbox');
    fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: true });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('空输入不触发 onSubmit', () => {
    const onSubmit = vi.fn();
    render(
      <PromptInput value="   " onChange={vi.fn()} onSubmit={onSubmit} />,
    );
    const textarea = screen.getByRole('textbox');
    fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: false });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('disabled 时不触发 onSubmit', () => {
    const onSubmit = vi.fn();
    render(
      <PromptInput
        value="test"
        onChange={vi.fn()}
        onSubmit={onSubmit}
        disabled
      />,
    );
    const textarea = screen.getByRole('textbox');
    fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: false });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('bash 模式下显示红色边框样式', () => {
    render(
      <PromptInput value="!ls" onChange={vi.fn()} onSubmit={vi.fn()} />,
    );
    const container = screen.getByTestId('prompt-input');
    expect(container.className).toContain('border-red-300');
  });

  it('显示自定义 placeholder', () => {
    render(
      <PromptInput
        value=""
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        placeholder="自定义提示"
      />,
    );
    expect(screen.getByPlaceholderText('自定义提示')).toBeInTheDocument();
  });
});
