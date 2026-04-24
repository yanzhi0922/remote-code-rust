import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MCPStdioServerMenu } from './MCPStdioServerMenu';

afterEach(() => { cleanup(); });

const defaults = {
  serverName: 'test-stdio-server',
  command: 'npx',
  args: ['-y', 'mcp-server'],
};

describe('MCPStdioServerMenu', () => {
  it('renders with data-testid', () => {
    render(<MCPStdioServerMenu {...defaults} />);
    expect(screen.getByTestId('mcp-stdio-server-menu')).toBeTruthy();
  });

  it('displays server name and command', () => {
    render(<MCPStdioServerMenu {...defaults} />);
    expect(screen.getByText('test-stdio-server')).toBeTruthy();
    expect(screen.getByText('npx -y mcp-server')).toBeTruthy();
  });

  it('displays cwd when provided', () => {
    render(<MCPStdioServerMenu {...defaults} cwd="/home/user/project" />);
    expect(screen.getByText(/\/home\/user\/project/)).toBeTruthy();
  });

  it('does not display cwd when not provided', () => {
    render(<MCPStdioServerMenu {...defaults} />);
    expect(screen.queryByText(/📂/)).toBeNull();
  });

  it('opens dropdown on menu button click', () => {
    const onConnect = vi.fn();
    render(<MCPStdioServerMenu {...defaults} onConnect={onConnect} />);
    fireEvent.click(screen.getByTitle('菜单'));
    expect(screen.getByText('连接')).toBeTruthy();
  });

  it('calls onConnect when connect clicked', () => {
    const onConnect = vi.fn();
    render(<MCPStdioServerMenu {...defaults} onConnect={onConnect} />);
    fireEvent.click(screen.getByTitle('菜单'));
    fireEvent.click(screen.getByText('连接'));
    expect(onConnect).toHaveBeenCalledOnce();
  });

  it('calls onDisconnect when disconnect clicked', () => {
    const onDisconnect = vi.fn();
    render(<MCPStdioServerMenu {...defaults} onDisconnect={onDisconnect} />);
    fireEvent.click(screen.getByTitle('菜单'));
    fireEvent.click(screen.getByText('断开'));
    expect(onDisconnect).toHaveBeenCalledOnce();
  });

  it('calls onRemove when remove clicked', () => {
    const onRemove = vi.fn();
    render(<MCPStdioServerMenu {...defaults} onRemove={onRemove} />);
    fireEvent.click(screen.getByTitle('菜单'));
    fireEvent.click(screen.getByText('删除'));
    expect(onRemove).toHaveBeenCalledOnce();
  });

  it('does not show connect when onConnect not provided', () => {
    render(<MCPStdioServerMenu {...defaults} />);
    fireEvent.click(screen.getByTitle('菜单'));
    expect(screen.queryByText('连接')).toBeNull();
  });

  it('applies custom className', () => {
    render(<MCPStdioServerMenu {...defaults} className="mt-4" />);
    expect(screen.getByTestId('mcp-stdio-server-menu').classList.contains('mt-4')).toBe(true);
  });

  it('displays command without args', () => {
    render(<MCPStdioServerMenu serverName="srv" command="node" />);
    expect(screen.getByText('node')).toBeTruthy();
  });
});
