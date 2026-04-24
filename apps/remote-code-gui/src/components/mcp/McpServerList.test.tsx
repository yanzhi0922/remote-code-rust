import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { McpServerInfo } from '../../lib/types';
import { McpServerList } from './McpServerList';

const servers: McpServerInfo[] = [
  {
    name: 'filesystem',
    enabled: true,
    transport: 'stdio',
    config_path: '/path/config.json',
    command: 'node',
    url: null,
    args: ['server.js'],
    cwd: null,
    env_keys: [],
    metadata_keys: [],
    startup_timeout_secs: null,
    request_timeout_secs: null,
    live: null,
  },
  {
    name: 'remote-api',
    enabled: true,
    transport: 'http',
    config_path: '/path/config.json',
    command: null,
    url: 'http://localhost:8080',
    args: [],
    cwd: null,
    env_keys: [],
    metadata_keys: [],
    startup_timeout_secs: null,
    request_timeout_secs: null,
    live: null,
  },
];

describe('McpServerList', () => {
  beforeEach(() => {
    cleanup();
  });

  afterEach(() => {
    cleanup();
  });

  it('renders all servers', () => {
    render(<McpServerList servers={servers} onSelectServer={vi.fn()} onAddServer={vi.fn()} />);
    expect(screen.getByText('filesystem')).toBeInTheDocument();
    expect(screen.getByText('remote-api')).toBeInTheDocument();
  });

  it('shows empty state when no servers', () => {
    render(<McpServerList servers={[]} onSelectServer={vi.fn()} onAddServer={vi.fn()} />);
    expect(screen.getByText('暂无 MCP 服务器')).toBeInTheDocument();
  });

  it('calls onAddServer when add button clicked', () => {
    const onAddServer = vi.fn();
    render(<McpServerList servers={[]} onSelectServer={vi.fn()} onAddServer={onAddServer} />);
    fireEvent.click(screen.getByTestId('mcp-add-server-btn'));
    expect(onAddServer).toHaveBeenCalledTimes(1);
  });

  it('calls onSelectServer when a server card is clicked', () => {
    const onSelectServer = vi.fn();
    render(<McpServerList servers={servers} onSelectServer={onSelectServer} onAddServer={vi.fn()} />);
    fireEvent.click(screen.getByTestId('mcp-server-card-filesystem'));
    expect(onSelectServer).toHaveBeenCalledWith('filesystem');
  });

  it('filters servers by name using search input', () => {
    render(<McpServerList servers={servers} onSelectServer={vi.fn()} onAddServer={vi.fn()} />);
    const searchInput = screen.getByTestId('mcp-server-search');
    fireEvent.change(searchInput, { target: { value: 'file' } });
    expect(screen.getByText('filesystem')).toBeInTheDocument();
    expect(screen.queryByText('remote-api')).not.toBeInTheDocument();
  });

  it('filters servers by transport type', () => {
    render(<McpServerList servers={servers} onSelectServer={vi.fn()} onAddServer={vi.fn()} />);
    const searchInput = screen.getByTestId('mcp-server-search');
    fireEvent.change(searchInput, { target: { value: 'http' } });
    expect(screen.queryByText('filesystem')).not.toBeInTheDocument();
    expect(screen.getByText('remote-api')).toBeInTheDocument();
  });

  it('shows empty state when search matches nothing', () => {
    render(<McpServerList servers={servers} onSelectServer={vi.fn()} onAddServer={vi.fn()} />);
    const searchInput = screen.getByTestId('mcp-server-search');
    fireEvent.change(searchInput, { target: { value: 'nonexistent' } });
    expect(screen.getByText('暂无 MCP 服务器')).toBeInTheDocument();
  });
});
