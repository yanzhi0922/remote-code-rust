import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { McpServerInfo } from '../../lib/types';
import { McpServerMenu } from './McpServerMenu';

const stdioServer: McpServerInfo = {
  name: 'test-server',
  enabled: true,
  transport: 'stdio',
  config_path: '/path/config.json',
  command: 'node',
  url: null,
  args: ['server.js', '--port', '3000'],
  cwd: '/home/user',
  env_keys: ['API_KEY'],
  metadata_keys: [],
  startup_timeout_secs: 30,
  request_timeout_secs: 60,
  live: null,
};

const connectedServer: McpServerInfo = {
  ...stdioServer,
  live: {
    status: 'connected',
    protocol_version: '2024-11-05',
    peer_name: 'test-peer',
    peer_version: '1.2.0',
    tool_count: 3,
    tools: [],
    error: null,
  },
};

const errorServer: McpServerInfo = {
  ...stdioServer,
  live: {
    status: 'error',
    protocol_version: null,
    peer_name: null,
    peer_version: null,
    tool_count: 0,
    tools: [],
    error: 'Connection refused',
  },
};

describe('McpServerMenu', () => {
  beforeEach(() => cleanup());
  afterEach(() => cleanup());

  it('renders server details', () => {
    render(<McpServerMenu server={stdioServer} onConnect={vi.fn()} onDisconnect={vi.fn()} onToggle={vi.fn()} onRemove={vi.fn()} onViewTools={vi.fn()} />);
    expect(screen.getByText('test-server')).toBeInTheDocument();
    expect(screen.getByText(/node server\.js --port 3000/)).toBeInTheDocument();
    expect(screen.getByText('/home/user')).toBeInTheDocument();
    expect(screen.getByText('API_KEY')).toBeInTheDocument();
  });

  it('shows connect button when disconnected', () => {
    render(<McpServerMenu server={stdioServer} onConnect={vi.fn()} onDisconnect={vi.fn()} onToggle={vi.fn()} onRemove={vi.fn()} onViewTools={vi.fn()} />);
    expect(screen.getByTestId('mcp-connect-btn')).toBeInTheDocument();
  });

  it('shows disconnect button when connected', () => {
    render(<McpServerMenu server={connectedServer} onConnect={vi.fn()} onDisconnect={vi.fn()} onToggle={vi.fn()} onRemove={vi.fn()} onViewTools={vi.fn()} />);
    expect(screen.getByTestId('mcp-disconnect-btn')).toBeInTheDocument();
  });

  it('shows live info when connected', () => {
    render(<McpServerMenu server={connectedServer} onConnect={vi.fn()} onDisconnect={vi.fn()} onToggle={vi.fn()} onRemove={vi.fn()} onViewTools={vi.fn()} />);
    expect(screen.getByText('连接信息')).toBeInTheDocument();
    expect(screen.getByText(/2024-11-05/)).toBeInTheDocument();
    expect(screen.getByText(/test-peer v1\.2\.0/)).toBeInTheDocument();
  });

  it('shows error info when server has error', () => {
    render(<McpServerMenu server={errorServer} onConnect={vi.fn()} onDisconnect={vi.fn()} onToggle={vi.fn()} onRemove={vi.fn()} onViewTools={vi.fn()} />);
    expect(screen.getByText(/Connection refused/)).toBeInTheDocument();
  });

  it('requires confirmation for remove', () => {
    const onRemove = vi.fn();
    render(<McpServerMenu server={stdioServer} onConnect={vi.fn()} onDisconnect={vi.fn()} onToggle={vi.fn()} onRemove={onRemove} onViewTools={vi.fn()} />);
    fireEvent.click(screen.getByTestId('mcp-remove-btn'));
    expect(screen.getByTestId('mcp-confirm-remove')).toBeInTheDocument();
    expect(onRemove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('mcp-confirm-remove-yes'));
    expect(onRemove).toHaveBeenCalledTimes(1);
  });

  it('calls onToggle with correct enabled value', () => {
    const onToggle = vi.fn();
    render(<McpServerMenu server={stdioServer} onConnect={vi.fn()} onDisconnect={vi.fn()} onToggle={onToggle} onRemove={vi.fn()} onViewTools={vi.fn()} />);
    fireEvent.click(screen.getByTestId('mcp-toggle-btn'));
    expect(onToggle).toHaveBeenCalledWith(false);
  });

  it('calls onViewTools when tools button clicked', () => {
    const onViewTools = vi.fn();
    render(<McpServerMenu server={stdioServer} onConnect={vi.fn()} onDisconnect={vi.fn()} onToggle={vi.fn()} onRemove={vi.fn()} onViewTools={onViewTools} />);
    fireEvent.click(screen.getByTestId('mcp-view-tools-btn'));
    expect(onViewTools).toHaveBeenCalledTimes(1);
  });
});
