import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { McpServerInfo } from '../../lib/types';
import { McpServerCard } from './McpServerCard';

const baseServer: McpServerInfo = {
  name: 'test-server',
  enabled: true,
  transport: 'stdio',
  config_path: '/path/to/config.json',
  command: 'node',
  url: null,
  args: ['server.js'],
  cwd: null,
  env_keys: [],
  metadata_keys: [],
  startup_timeout_secs: null,
  request_timeout_secs: null,
  live: null,
};

describe('McpServerCard', () => {
  beforeEach(() => {
    cleanup();
  });

  afterEach(() => {
    cleanup();
  });

  it('renders server name and transport type', () => {
    render(<McpServerCard server={baseServer} onClick={() => {}} />);
    expect(screen.getByText('test-server')).toBeInTheDocument();
    expect(screen.getByText('stdio')).toBeInTheDocument();
  });

  it('shows connected status with green dot', () => {
    const server: McpServerInfo = {
      ...baseServer,
      live: {
        status: 'connected',
        protocol_version: '1.0',
        peer_name: 'test',
        peer_version: '1.0',
        tool_count: 3,
        tools: [],
        error: null,
      },
    };
    render(<McpServerCard server={server} onClick={() => {}} />);
    expect(screen.getByText('已连接')).toBeInTheDocument();
    const dot = screen.getByText('已连接').parentElement!.querySelector('.rounded-full');
    expect(dot).toHaveClass('bg-emerald-500');
  });

  it('shows error status with red dot', () => {
    const server: McpServerInfo = {
      ...baseServer,
      live: { status: 'error', protocol_version: null, peer_name: null, peer_version: null, tool_count: 0, tools: [], error: 'fail' },
    };
    render(<McpServerCard server={server} onClick={() => {}} />);
    expect(screen.getByText('错误')).toBeInTheDocument();
  });

  it('shows disabled label when server is disabled', () => {
    const server: McpServerInfo = { ...baseServer, enabled: false };
    render(<McpServerCard server={server} onClick={() => {}} />);
    expect(screen.getByText('已禁用')).toBeInTheDocument();
  });

  it('shows tool count badge when tools are available', () => {
    const server: McpServerInfo = {
      ...baseServer,
      live: {
        status: 'connected',
        protocol_version: '1.0',
        peer_name: 'test',
        peer_version: '1.0',
        tool_count: 5,
        tools: [],
        error: null,
      },
    };
    render(<McpServerCard server={server} onClick={() => {}} />);
    expect(screen.getByText('5')).toBeInTheDocument();
  });

  it('calls onClick when clicked', () => {
    const onClick = vi.fn();
    render(<McpServerCard server={baseServer} onClick={onClick} />);
    fireEvent.click(screen.getByTestId('mcp-server-card-test-server'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('shows selected state when selected prop is true', () => {
    render(<McpServerCard server={baseServer} onClick={() => {}} selected={true} />);
    const card = screen.getByTestId('mcp-server-card-test-server');
    expect(card).toHaveClass('border-emerald-300');
  });

  it('displays http transport type correctly', () => {
    const server: McpServerInfo = { ...baseServer, transport: 'http', command: null, url: 'http://localhost:8080' };
    render(<McpServerCard server={server} onClick={() => {}} />);
    expect(screen.getByText('HTTP')).toBeInTheDocument();
  });
});
