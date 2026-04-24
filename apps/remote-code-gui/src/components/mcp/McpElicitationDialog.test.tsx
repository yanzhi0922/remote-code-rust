import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { McpElicitationDialog } from './McpElicitationDialog';

const fields = [
  { name: 'username', label: '用户名', type: 'text' as const },
  { name: 'password', label: '密码', type: 'password' as const },
  { name: 'role', label: '角色', type: 'select' as const, options: ['admin', 'user'] },
];

describe('McpElicitationDialog', () => {
  beforeEach(() => cleanup());
  afterEach(() => cleanup());

  it('renders nothing when visible is false', () => {
    render(<McpElicitationDialog visible={false} title="Test" message="msg" fields={fields} onSubmit={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.queryByTestId('mcp-elicitation-dialog')).not.toBeInTheDocument();
  });

  it('renders title and message when visible', () => {
    render(<McpElicitationDialog visible={true} title="认证请求" message="请输入凭据" fields={fields} onSubmit={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByText('认证请求')).toBeInTheDocument();
    expect(screen.getByText('请输入凭据')).toBeInTheDocument();
  });

  it('renders text input fields', () => {
    render(<McpElicitationDialog visible={true} title="Test" message="" fields={[fields[0]]} onSubmit={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByTestId('mcp-elicitation-field-username')).toBeInTheDocument();
  });

  it('renders password input fields', () => {
    render(<McpElicitationDialog visible={true} title="Test" message="" fields={[fields[1]]} onSubmit={vi.fn()} onCancel={vi.fn()} />);
    const input = screen.getByTestId('mcp-elicitation-field-password');
    expect(input).toHaveAttribute('type', 'password');
  });

  it('renders select fields with options', () => {
    render(<McpElicitationDialog visible={true} title="Test" message="" fields={[fields[2]]} onSubmit={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByText('admin')).toBeInTheDocument();
    expect(screen.getByText('user')).toBeInTheDocument();
  });

  it('calls onCancel when cancel button clicked', () => {
    const onCancel = vi.fn();
    render(<McpElicitationDialog visible={true} title="Test" message="" fields={fields} onSubmit={vi.fn()} onCancel={onCancel} />);
    fireEvent.click(screen.getByTestId('mcp-elicitation-cancel'));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('calls onSubmit with field values', () => {
    const onSubmit = vi.fn();
    render(<McpElicitationDialog visible={true} title="Test" message="" fields={[fields[0]]} onSubmit={onSubmit} onCancel={vi.fn()} />);
    fireEvent.change(screen.getByTestId('mcp-elicitation-field-username'), { target: { value: 'john' } });
    fireEvent.click(screen.getByTestId('mcp-elicitation-submit'));
    expect(onSubmit).toHaveBeenCalledWith({ username: 'john' });
  });

  it('calls onCancel when close button clicked', () => {
    const onCancel = vi.fn();
    render(<McpElicitationDialog visible={true} title="Test" message="" fields={fields} onSubmit={vi.fn()} onCancel={onCancel} />);
    fireEvent.click(screen.getByTestId('mcp-elicitation-close'));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
