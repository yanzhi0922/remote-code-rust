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

  // ── 斜杠命令 typeahead ──

  it('输入 / 时显示斜杠命令建议', () => {
    render(<PromptInput value="/" onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(screen.getByTestId('slash-commands')).toBeInTheDocument();
    expect(screen.getByTestId('slash-command-help')).toBeInTheDocument();
  });

  it('输入 /cl 时过滤命令列表', () => {
    render(<PromptInput value="/cl" onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(screen.getByTestId('slash-commands')).toBeInTheDocument();
    expect(screen.getByTestId('slash-command-clear')).toBeInTheDocument();
  });

  it('点击斜杠命令时填入命令', () => {
    const onChange = vi.fn();
    render(<PromptInput value="/he" onChange={onChange} onSubmit={vi.fn()} />);
    fireEvent.click(screen.getByTestId('slash-command-help'));
    expect(onChange).toHaveBeenCalledWith('/help ');
  });

  it('Escape 关闭斜杠命令列表', () => {
    render(<PromptInput value="/" onChange={vi.fn()} onSubmit={vi.fn()} />);
    const textarea = screen.getByRole('textbox');
    fireEvent.keyDown(textarea, { key: 'Escape' });
    expect(screen.queryByTestId('slash-commands')).not.toBeInTheDocument();
  });

  it('非斜杠输入不显示命令列表', () => {
    render(<PromptInput value="hello" onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(screen.queryByTestId('slash-commands')).not.toBeInTheDocument();
  });

  // ── @ 提及补全 ──

  it('输入 @ 时显示提及建议', () => {
    render(<PromptInput value="@" onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(screen.getByTestId('mention-suggestions')).toBeInTheDocument();
  });

  it('点击提及建议时填入', () => {
    const onChange = vi.fn();
    render(<PromptInput value="请查看 @" onChange={onChange} onSubmit={vi.fn()} />);
    fireEvent.click(screen.getByTestId('mention-file'));
    expect(onChange).toHaveBeenCalledWith('请查看 @文件 ');
  });

  // ── 历史搜索 ──

  it('提交后保存历史', () => {
    const onSubmit = vi.fn();
    const onChange = vi.fn();
    render(
      <PromptInput value="first input" onChange={onChange} onSubmit={onSubmit} />,
    );
    const textarea = screen.getByRole('textbox');
    // 提交
    fireEvent.keyDown(textarea, { key: 'Enter' });
    expect(onSubmit).toHaveBeenCalledWith('first input');
  });

  // ── 底部工具栏 ──

  it('显示模型名称', () => {
    render(
      <PromptInput
        value=""
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        modelName="gpt-4"
      />,
    );
    expect(screen.getByTestId('toolbar-model')).toBeInTheDocument();
    expect(screen.getByText('gpt-4')).toBeInTheDocument();
  });

  it('点击模型名称调用 onModelSelect', () => {
    const onModelSelect = vi.fn();
    render(
      <PromptInput
        value=""
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        modelName="gpt-4"
        onModelSelect={onModelSelect}
      />,
    );
    fireEvent.click(screen.getByTestId('toolbar-model'));
    expect(onModelSelect).toHaveBeenCalled();
  });

  it('显示权限模式', () => {
    render(
      <PromptInput
        value=""
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        permissionMode="auto-allow"
      />,
    );
    expect(screen.getByTestId('toolbar-permission')).toBeInTheDocument();
    expect(screen.getByText('Auto')).toBeInTheDocument();
  });

  it('显示 ask 权限模式', () => {
    render(
      <PromptInput
        value=""
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        permissionMode="ask"
      />,
    );
    expect(screen.getByText('Ask')).toBeInTheDocument();
  });

  it('显示 deny 权限模式', () => {
    render(
      <PromptInput
        value=""
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        permissionMode="deny"
      />,
    );
    expect(screen.getByText('Deny')).toBeInTheDocument();
  });

  it('显示 token 用量', () => {
    render(
      <PromptInput
        value=""
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        tokenUsage={{ input: 1500, output: 500 }}
      />,
    );
    expect(screen.getByTestId('toolbar-tokens')).toBeInTheDocument();
  });

  it('显示 thinking toggle', () => {
    const onThinkingToggle = vi.fn();
    render(
      <PromptInput
        value=""
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        thinkingEnabled={true}
        onThinkingToggle={onThinkingToggle}
      />,
    );
    expect(screen.getByTestId('toolbar-thinking')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('toolbar-thinking'));
    expect(onThinkingToggle).toHaveBeenCalled();
  });

  it('显示 fast mode 指示器', () => {
    render(
      <PromptInput
        value=""
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        fastMode={true}
      />,
    );
    expect(screen.getByTestId('toolbar-fast-mode')).toBeInTheDocument();
  });

  it('显示字数统计', () => {
    render(
      <PromptInput value="hello" onChange={vi.fn()} onSubmit={vi.fn()} />,
    );
    expect(screen.getByTestId('toolbar-char-count')).toBeInTheDocument();
    expect(screen.getByTestId('toolbar-char-count').textContent).toBe('5');
  });

  // ── 工具栏折叠/展开 ──

  it('折叠工具栏', () => {
    render(<PromptInput value="" onChange={vi.fn()} onSubmit={vi.fn()} />);
    fireEvent.click(screen.getByTestId('toolbar-collapse'));
    expect(screen.queryByTestId('prompt-toolbar')).not.toBeInTheDocument();
    expect(screen.getByTestId('toolbar-expand')).toBeInTheDocument();
  });

  it('展开工具栏', () => {
    render(<PromptInput value="" onChange={vi.fn()} onSubmit={vi.fn()} />);
    fireEvent.click(screen.getByTestId('toolbar-collapse'));
    fireEvent.click(screen.getByTestId('toolbar-expand'));
    expect(screen.getByTestId('prompt-toolbar')).toBeInTheDocument();
  });

  // ── 加载状态 ──

  it('isLoading 时显示加载指示器', () => {
    render(
      <PromptInput
        value=""
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        isLoading={true}
      />,
    );
    expect(screen.getByTestId('loading-indicator')).toBeInTheDocument();
  });

  it('isLoading 时禁用提交', () => {
    const onSubmit = vi.fn();
    render(
      <PromptInput
        value="test"
        onChange={vi.fn()}
        onSubmit={onSubmit}
        isLoading={true}
      />,
    );
    const textarea = screen.getByRole('textbox');
    fireEvent.keyDown(textarea, { key: 'Enter' });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  // ── 无工具栏时不显示多余元素 ──

  it('无 modelName 时不显示模型按钮', () => {
    render(<PromptInput value="" onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(screen.queryByTestId('toolbar-model')).not.toBeInTheDocument();
  });

  it('无 permissionMode 时不显示权限指示器', () => {
    render(<PromptInput value="" onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(screen.queryByTestId('toolbar-permission')).not.toBeInTheDocument();
  });

  it('无 tokenUsage 时不显示 token 用量', () => {
    render(<PromptInput value="" onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(screen.queryByTestId('toolbar-tokens')).not.toBeInTheDocument();
  });

  it('无 fastMode 时不显示 fast mode 指示器', () => {
    render(<PromptInput value="" onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(screen.queryByTestId('toolbar-fast-mode')).not.toBeInTheDocument();
  });
});
