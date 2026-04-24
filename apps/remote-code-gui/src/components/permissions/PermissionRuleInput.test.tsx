import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PermissionRuleInput } from './PermissionRuleInput';

describe('PermissionRuleInput', () => {
  afterEach(cleanup);

  it('renders all input fields and buttons', () => {
    render(<PermissionRuleInput onSubmit={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByLabelText('工具名')).toBeInTheDocument();
    expect(screen.getByLabelText('规则内容')).toBeInTheDocument();
    expect(screen.getByLabelText('行为')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '添加' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '取消' })).toBeInTheDocument();
  });

  it('calls onSubmit with correct values', () => {
    const onSubmit = vi.fn();
    render(<PermissionRuleInput onSubmit={onSubmit} onCancel={vi.fn()} />);

    fireEvent.change(screen.getByLabelText('工具名'), { target: { value: 'Bash' } });
    fireEvent.change(screen.getByLabelText('规则内容'), { target: { value: 'npm test' } });
    fireEvent.change(screen.getByLabelText('行为'), { target: { value: 'deny' } });
    fireEvent.click(screen.getByRole('button', { name: '添加' }));

    expect(onSubmit).toHaveBeenCalledWith({
      tool_name: 'Bash',
      rule_content: 'npm test',
      behavior: 'deny',
    });
  });

  it('shows validation errors for empty fields', () => {
    render(<PermissionRuleInput onSubmit={vi.fn()} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: '添加' }));
    expect(screen.getByText('工具名不能为空')).toBeInTheDocument();
    expect(screen.getByText('规则内容不能为空')).toBeInTheDocument();
  });

  it('calls onCancel when cancel button is clicked', () => {
    const onCancel = vi.fn();
    render(<PermissionRuleInput onSubmit={vi.fn()} onCancel={onCancel} />);
    fireEvent.click(screen.getByRole('button', { name: '取消' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('submits on Enter key press', () => {
    const onSubmit = vi.fn();
    render(<PermissionRuleInput onSubmit={onSubmit} onCancel={vi.fn()} />);

    fireEvent.change(screen.getByLabelText('工具名'), { target: { value: 'Bash' } });
    fireEvent.change(screen.getByLabelText('规则内容'), { target: { value: 'ls' } });

    const contentInput = screen.getByLabelText('规则内容');
    fireEvent.keyDown(contentInput, { key: 'Enter' });

    expect(onSubmit).toHaveBeenCalledWith({
      tool_name: 'Bash',
      rule_content: 'ls',
      behavior: 'allow',
    });
  });

  it('defaults behavior to allow', () => {
    render(<PermissionRuleInput onSubmit={vi.fn()} onCancel={vi.fn()} />);
    const select = screen.getByLabelText('行为') as HTMLSelectElement;
    expect(select.value).toBe('allow');
  });
});
