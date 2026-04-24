import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { McpServerInfo } from '../../lib/types';
import { MCPSettings } from './MCPSettings';

function makeServer(overrides: Partial<McpServerInfo> = {}): McpServerInfo {
  return {
    name: 'test-server',
    enabled: true,
    transport: 'stdio',
    config_path: '/test/config.json',
    command: 'npx test-server',
    url: null,
    args: [],
    cwd: null,
    env_keys: [],
    metadata_keys: [],
    startup_timeout_secs: null,
    request_timeout_secs: null,
    live: {
      status: 'connected',
      protocol_version: '1.0',
      peer_name: 'test',
      peer_version: '1.0',
      tool_count: 3,
      tools: [
        { name: 'tool-a', description: 'Tool A' },
        { name: 'tool-b', description: 'Tool B' },
        { name: 'tool-c', description: null },
      ],
      error: null,
    },
    ...overrides,
  };
}

describe('MCPSettings', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<MCPSettings />);
    expect(screen.getByTestId('mcp-settings')).toBeInTheDocument();
  });

  it('shows timeout input', () => {
    render(<MCPSettings defaultTimeout={60} />);
    const input = screen.getByTestId('mcp-timeout-input') as HTMLInputElement;
    expect(input.value).toBe('60');
  });

  it('calls onTimeoutChange when timeout changes', () => {
    const fn = vi.fn();
    render(<MCPSettings onTimeoutChange={fn} />);
    fireEvent.change(screen.getByTestId('mcp-timeout-input'), { target: { value: '45' } });
    expect(fn).toHaveBeenCalledWith(45);
  });

  // ── 服务器列表 ──

  it('shows empty state when no servers', () => {
    render(<MCPSettings servers={[]} />);
    expect(screen.getByText('暂无 MCP 服务器配置')).toBeInTheDocument();
  });

  it('renders server cards', () => {
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} />);
    expect(screen.getByTestId('mcp-server-test-server')).toBeInTheDocument();
  });

  it('shows server name and transport type', () => {
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} />);
    expect(screen.getByText('test-server')).toBeInTheDocument();
    // STDIO appears in both tab and card, use getAllByText
    expect(screen.getAllByText('STDIO').length).toBeGreaterThanOrEqual(1);
  });

  it('shows connected status', () => {
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} />);
    expect(screen.getByText('已连接')).toBeInTheDocument();
  });

  it('shows tool count', () => {
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} />);
    expect(screen.getByText('3 个工具')).toBeInTheDocument();
  });

  it('shows disconnected status when no live info', () => {
    const servers = [makeServer({ live: null })];
    render(<MCPSettings servers={servers} />);
    expect(screen.getByText('未连接')).toBeInTheDocument();
  });

  // ── 标签页过滤 ──

  it('filters servers by stdio tab', () => {
    const servers = [
      makeServer({ name: 'stdio-srv', transport: 'stdio' }),
      makeServer({ name: 'http-srv', transport: 'http', url: 'http://example.com', command: null }),
    ];
    render(<MCPSettings servers={servers} />);
    fireEvent.click(screen.getByTestId('mcp-tab-stdio'));
    expect(screen.getByTestId('mcp-server-stdio-srv')).toBeInTheDocument();
    expect(screen.queryByTestId('mcp-server-http-srv')).not.toBeInTheDocument();
  });

  it('shows all servers in all tab', () => {
    const servers = [
      makeServer({ name: 'stdio-srv', transport: 'stdio' }),
      makeServer({ name: 'http-srv', transport: 'http', url: 'http://example.com', command: null }),
    ];
    render(<MCPSettings servers={servers} />);
    // Default is 'all' tab
    expect(screen.getByTestId('mcp-server-stdio-srv')).toBeInTheDocument();
    expect(screen.getByTestId('mcp-server-http-srv')).toBeInTheDocument();
  });

  // ── 启用/禁用 ──

  it('calls onToggleServer when toggle clicked', () => {
    const fn = vi.fn();
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} onToggleServer={fn} />);
    fireEvent.click(screen.getByTestId('mcp-toggle-test-server'));
    expect(fn).toHaveBeenCalledWith('test-server', false);
  });

  // ── 删除 ──

  it('shows confirm delete on first click', () => {
    const fn = vi.fn();
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} onRemoveServer={fn} />);
    fireEvent.click(screen.getByTestId('mcp-delete-test-server'));
    expect(screen.getByText('再次点击确认删除')).toBeInTheDocument();
  });

  it('calls onRemoveServer on second click', () => {
    const fn = vi.fn();
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} onRemoveServer={fn} />);
    fireEvent.click(screen.getByTestId('mcp-delete-test-server'));
    fireEvent.click(screen.getByTestId('mcp-delete-test-server'));
    expect(fn).toHaveBeenCalledWith('test-server');
  });

  // ── 服务器详情导航 ──

  it('navigates to server detail view', () => {
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} />);
    fireEvent.click(screen.getByTestId('mcp-server-detail-test-server'));
    expect(screen.getByTestId('mcp-server-detail-view')).toBeInTheDocument();
  });

  it('shows tool list in server detail', () => {
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} />);
    fireEvent.click(screen.getByTestId('mcp-server-detail-test-server'));
    expect(screen.getByTestId('mcp-tool-tool-a')).toBeInTheDocument();
    expect(screen.getByTestId('mcp-tool-tool-b')).toBeInTheDocument();
  });

  it('navigates back from server detail', () => {
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} />);
    fireEvent.click(screen.getByTestId('mcp-server-detail-test-server'));
    fireEvent.click(screen.getByTestId('mcp-back-to-list'));
    expect(screen.getByTestId('mcp-server-list')).toBeInTheDocument();
  });

  // ── 工具详情导航 ──

  it('navigates to tool detail view', () => {
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} />);
    fireEvent.click(screen.getByTestId('mcp-server-detail-test-server'));
    fireEvent.click(screen.getByTestId('mcp-tool-tool-a'));
    expect(screen.getByTestId('mcp-tool-detail-view')).toBeInTheDocument();
  });

  it('shows tool description in detail', () => {
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} />);
    fireEvent.click(screen.getByTestId('mcp-server-detail-test-server'));
    fireEvent.click(screen.getByTestId('mcp-tool-tool-a'));
    expect(screen.getByText('Tool A')).toBeInTheDocument();
  });

  it('navigates back from tool detail to server detail', () => {
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} />);
    fireEvent.click(screen.getByTestId('mcp-server-detail-test-server'));
    fireEvent.click(screen.getByTestId('mcp-tool-tool-a'));
    fireEvent.click(screen.getByTestId('mcp-back-to-server'));
    expect(screen.getByTestId('mcp-server-detail-view')).toBeInTheDocument();
  });

  // ── 添加服务器 ──

  it('shows add server button when onAddServer provided', () => {
    render(<MCPSettings onAddServer={vi.fn()} />);
    expect(screen.getByTestId('mcp-add-server')).toBeInTheDocument();
  });

  it('does not show add server button without onAddServer', () => {
    render(<MCPSettings />);
    expect(screen.queryByTestId('mcp-add-server')).not.toBeInTheDocument();
  });

  it('navigates to add server form', () => {
    render(<MCPSettings onAddServer={vi.fn()} />);
    fireEvent.click(screen.getByTestId('mcp-add-server'));
    expect(screen.getByTestId('mcp-add-server-form')).toBeInTheDocument();
  });

  it('calls onAddServer with form data', () => {
    const fn = vi.fn();
    render(<MCPSettings onAddServer={fn} />);
    fireEvent.click(screen.getByTestId('mcp-add-server'));
    fireEvent.change(screen.getByTestId('mcp-new-name'), { target: { value: 'my-server' } });
    fireEvent.click(screen.getByTestId('mcp-new-transport-http'));
    fireEvent.change(screen.getByTestId('mcp-new-url'), { target: { value: 'http://example.com' } });
    fireEvent.click(screen.getByTestId('mcp-new-submit'));
    expect(fn).toHaveBeenCalledWith({
      name: 'my-server',
      transport: 'http',
      command: undefined,
      url: 'http://example.com',
      scope: 'project',
    });
  });

  it('disables submit when name is empty', () => {
    const fn = vi.fn();
    render(<MCPSettings onAddServer={fn} />);
    fireEvent.click(screen.getByTestId('mcp-add-server'));
    fireEvent.click(screen.getByTestId('mcp-new-submit'));
    expect(fn).not.toHaveBeenCalled();
  });

  // ── 服务器配置信息 ──

  it('shows config path in server detail', () => {
    const servers = [makeServer()];
    render(<MCPSettings servers={servers} />);
    fireEvent.click(screen.getByTestId('mcp-server-detail-test-server'));
    expect(screen.getByText('/test/config.json')).toBeInTheDocument();
  });

  it('shows error status', () => {
    const servers = [makeServer({
      live: { status: 'error', protocol_version: null, peer_name: null, peer_version: null, tool_count: 0, tools: [], error: 'Connection refused' },
    })];
    render(<MCPSettings servers={servers} />);
    expect(screen.getByText('错误')).toBeInTheDocument();
  });
});
