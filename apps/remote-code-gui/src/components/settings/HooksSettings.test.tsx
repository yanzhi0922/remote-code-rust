import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { HookConfig } from './HooksSettings';
import { HooksSettings } from './HooksSettings';

const mockHooks: HookConfig[] = [
  { event: 'PreToolUse', command: 'echo "before"', enabled: true },
  { event: 'PostToolUse', command: 'echo "after"', enabled: false },
];

describe('HooksSettings', () => {
  afterEach(cleanup);

  it('renders section title', () => {
    render(<HooksSettings hooks={[]} onUpdate={vi.fn()} />);
    expect(screen.getByText('Hooks 配置')).toBeInTheDocument();
  });

  it('shows empty state when no hooks', () => {
    render(<HooksSettings hooks={[]} onUpdate={vi.fn()} />);
    expect(screen.getByText('暂无 Hooks 配置')).toBeInTheDocument();
  });

  it('renders hook rows when hooks are provided', () => {
    render(<HooksSettings hooks={mockHooks} onUpdate={vi.fn()} />);
    expect(screen.getByTestId('hook-row-0')).toBeInTheDocument();
    expect(screen.getByTestId('hook-row-1')).toBeInTheDocument();
  });

  it('calls onUpdate with new hook when add button is clicked', () => {
    const onUpdate = vi.fn();
    render(<HooksSettings hooks={[]} onUpdate={onUpdate} />);
    fireEvent.click(screen.getByTestId('add-hook-btn'));
    expect(onUpdate).toHaveBeenCalledWith([{ event: 'PreToolUse', command: '', enabled: true }]);
  });

  it('calls onUpdate without removed hook when delete is clicked', () => {
    const onUpdate = vi.fn();
    render(<HooksSettings hooks={mockHooks} onUpdate={onUpdate} />);
    fireEvent.click(screen.getByTestId('remove-hook-0'));
    expect(onUpdate).toHaveBeenCalledWith([mockHooks[1]]);
  });

  it('renders command inputs with correct values', () => {
    render(<HooksSettings hooks={mockHooks} onUpdate={vi.fn()} />);
    const inputs = screen.getAllByPlaceholderText('输入命令');
    expect(inputs[0]).toHaveValue('echo "before"');
    expect(inputs[1]).toHaveValue('echo "after"');
  });

  it('calls onUpdate when command input changes', () => {
    const onUpdate = vi.fn();
    render(<HooksSettings hooks={mockHooks} onUpdate={onUpdate} />);
    const inputs = screen.getAllByPlaceholderText('输入命令');
    fireEvent.change(inputs[0], { target: { value: 'new command' } });
    expect(onUpdate).toHaveBeenCalledWith([
      { event: 'PreToolUse', command: 'new command', enabled: true },
      mockHooks[1],
    ]);
  });
});
