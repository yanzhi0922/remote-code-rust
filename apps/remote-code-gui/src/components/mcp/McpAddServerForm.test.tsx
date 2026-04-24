import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { McpAddServerForm } from './McpAddServerForm';

describe('McpAddServerForm', () => {
  beforeEach(() => cleanup());
  afterEach(() => cleanup());

  it('renders form fields for stdio transport', () => {
    render(<McpAddServerForm onSubmit={vi.fn()} onCancel={vi.fn()} scope="profile" />);
    expect(screen.getByTestId('mcp-form-name')).toBeInTheDocument();
    expect(screen.getByTestId('mcp-form-command')).toBeInTheDocument();
    expect(screen.getByTestId('mcp-form-args')).toBeInTheDocument();
  });

  it('shows URL field when http transport selected', () => {
    render(<McpAddServerForm onSubmit={vi.fn()} onCancel={vi.fn()} scope="profile" />);
    fireEvent.click(screen.getByTestId('mcp-form-transport-http'));
    expect(screen.getByTestId('mcp-form-url')).toBeInTheDocument();
    expect(screen.queryByTestId('mcp-form-command')).not.toBeInTheDocument();
  });

  it('shows websocket URL field when websocket transport selected', () => {
    render(<McpAddServerForm onSubmit={vi.fn()} onCancel={vi.fn()} scope="profile" />);
    fireEvent.click(screen.getByTestId('mcp-form-transport-websocket'));
    expect(screen.getByTestId('mcp-form-url')).toBeInTheDocument();
  });

  it('validates required name field', () => {
    render(<McpAddServerForm onSubmit={vi.fn()} onCancel={vi.fn()} scope="profile" />);
    fireEvent.click(screen.getByTestId('mcp-form-submit'));
    expect(screen.getByText('名称不能为空')).toBeInTheDocument();
  });

  it('validates required command for stdio', () => {
    render(<McpAddServerForm onSubmit={vi.fn()} onCancel={vi.fn()} scope="profile" />);
    fireEvent.change(screen.getByTestId('mcp-form-name'), { target: { value: 'test' } });
    fireEvent.click(screen.getByTestId('mcp-form-submit'));
    expect(screen.getByText('stdio 类型必须填写命令')).toBeInTheDocument();
  });

  it('validates required url for http', () => {
    render(<McpAddServerForm onSubmit={vi.fn()} onCancel={vi.fn()} scope="profile" />);
    fireEvent.click(screen.getByTestId('mcp-form-transport-http'));
    fireEvent.change(screen.getByTestId('mcp-form-name'), { target: { value: 'test' } });
    fireEvent.click(screen.getByTestId('mcp-form-submit'));
    expect(screen.getByText('http 类型必须填写 URL')).toBeInTheDocument();
  });

  it('calls onSubmit with correct draft for stdio', () => {
    const onSubmit = vi.fn();
    render(<McpAddServerForm onSubmit={onSubmit} onCancel={vi.fn()} scope="profile" />);
    fireEvent.change(screen.getByTestId('mcp-form-name'), { target: { value: 'my-server' } });
    fireEvent.change(screen.getByTestId('mcp-form-command'), { target: { value: 'node server.js' } });
    fireEvent.change(screen.getByTestId('mcp-form-args'), { target: { value: '--port 3000' } });
    fireEvent.click(screen.getByTestId('mcp-form-submit'));
    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'my-server',
        transport: 'stdio',
        command: 'node server.js',
        args: ['--port', '3000'],
        scope: 'profile',
      }),
    );
  });

  it('calls onCancel when cancel button clicked', () => {
    const onCancel = vi.fn();
    render(<McpAddServerForm onSubmit={vi.fn()} onCancel={onCancel} scope="profile" />);
    fireEvent.click(screen.getByTestId('mcp-form-cancel'));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('includes disabled flag when checkbox is checked', () => {
    const onSubmit = vi.fn();
    render(<McpAddServerForm onSubmit={onSubmit} onCancel={vi.fn()} scope="profile" />);
    fireEvent.change(screen.getByTestId('mcp-form-name'), { target: { value: 'test' } });
    fireEvent.change(screen.getByTestId('mcp-form-command'), { target: { value: 'node x.js' } });
    fireEvent.click(screen.getByTestId('mcp-form-disabled'));
    fireEvent.click(screen.getByTestId('mcp-form-submit'));
    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({
        disabled: true,
      }),
    );
  });
});
