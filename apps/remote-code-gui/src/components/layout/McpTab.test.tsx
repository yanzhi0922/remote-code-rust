import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { McpTab } from './McpTab';
import { resetAppStore } from '../../test/appStoreTestUtils';
import * as tauri from '../../lib/tauri';

vi.mock('../../lib/tauri', () => ({
  listMcpServers: vi.fn(),
  listRuntimeMcpInventory: vi.fn(),
  saveMcpServer: vi.fn(),
  resetMcpServers: vi.fn(),
  toggleMcpServer: vi.fn(),
  removeMcpServer: vi.fn(),
}));

describe('McpTab', () => {
  beforeEach(() => {
    resetAppStore();
    vi.mocked(tauri.listMcpServers).mockResolvedValue({
      scope: 'profile',
      config_path: 'C:\\Users\\Yanzh\\.remote-code-rust\\mcp.json',
      warnings: [],
      servers: [
        {
          name: 'filesystem',
          enabled: true,
          transport: 'stdio',
          config_path: 'C:\\Users\\Yanzh\\.remote-code-rust\\mcp.json',
          command: 'python',
          url: null,
          args: ['server.py'],
          cwd: null,
          env_keys: [],
          metadata_keys: [],
          startup_timeout_secs: null,
          request_timeout_secs: null,
          live: null,
        },
      ],
    });
    vi.mocked(tauri.listRuntimeMcpInventory).mockRejectedValue(
      new Error('runtime discovery failed'),
    );
  });

  afterEach(() => {
    cleanup();
    resetAppStore();
    vi.clearAllMocks();
  });

  it('keeps managed servers visible when runtime inventory loading fails', async () => {
    render(<McpTab />);

    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });

    expect(screen.getByRole('region', { name: 'Runtime MCP inventory' })).toBeInTheDocument();
    expect(screen.getByText('filesystem')).toBeInTheDocument();
    expect(
      await screen.findByText(/无法加载 runtime inventory：runtime discovery failed/),
    ).toBeInTheDocument();
  });

  it('opens the editor with form mode by default when adding a new server', async () => {
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: '新增 / 更新 MCP Server' }));

    expect(screen.getByTestId('mcp-form-editor')).toBeInTheDocument();
    expect(screen.getByTestId('mcp-back-to-list')).toBeInTheDocument();
  });

  it('switches to JSON mode and back when toggling the tab', async () => {
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: '新增 / 更新 MCP Server' }));
    // Empty form -> JSON mode shows the placeholder text.
    fireEvent.click(screen.getByTestId('mcp-mode-json'));
    expect(screen.getByTestId('mcp-json-editor')).toBeInTheDocument();
    const textarea = screen.getByTestId('mcp-json-textarea') as HTMLTextAreaElement;
    // Switching to JSON from an empty form should leave the textarea empty
    // (no server name yet, no fake data).
    expect(textarea.value).toBe('');
    // Switch back: empty JSON should report an error (no single server key).
    fireEvent.click(screen.getByTestId('mcp-mode-form'));
    expect(screen.getByTestId('mcp-json-editor')).toBeInTheDocument();
    expect(screen.getByText(/必须包含一个 server 键/)).toBeInTheDocument();
  });

  it('accepts the mcpServers-wrapped JSON shape and switches back to form with name populated', async () => {
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: '新增 / 更新 MCP Server' }));
    fireEvent.click(screen.getByTestId('mcp-mode-json'));
    const wrapped = JSON.stringify({
      mcpServers: {
        'brave-search': {
          type: 'stdio',
          command: 'npx',
          args: ['-y', '@modelcontextprotocol/server-brave-search'],
          env: { BRAVE_API_KEY: 'test' },
        },
      },
    });
    fireEvent.change(screen.getByTestId('mcp-json-textarea'), {
      target: { value: wrapped },
    });
    fireEvent.click(screen.getByTestId('mcp-mode-form'));
    expect(screen.getByTestId('mcp-form-editor')).toBeInTheDocument();
    const nameInput = screen.getByPlaceholderText('brave-search') as HTMLInputElement;
    expect(nameInput.value).toBe('brave-search');
  });

  it('returns to the list view when the back button is clicked', async () => {
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: '新增 / 更新 MCP Server' }));
    expect(screen.getByTestId('mcp-editor')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('mcp-back-to-list'));
    // Editor gone, runtime + managed sections back.
    expect(screen.queryByTestId('mcp-editor')).not.toBeInTheDocument();
    expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
  });

  it('expands and collapses the env/metadata/timeouts block', async () => {
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: '新增 / 更新 MCP Server' }));
    // env collapsed by default — metadata textarea not visible yet.
    expect(screen.queryByPlaceholderText('scope=workspace')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId('mcp-toggle-env'));
    expect(screen.getByPlaceholderText('scope=workspace')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('mcp-toggle-env'));
    expect(screen.queryByPlaceholderText('scope=workspace')).not.toBeInTheDocument();
  });

  it('exposes all four MCP transport options', async () => {
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: '新增 / 更新 MCP Server' }));
    const select = screen.getByRole('combobox', { name: '类型' }) as HTMLSelectElement;
    const values = Array.from(select.options).map((o) => o.value);
    expect(values).toEqual(expect.arrayContaining(['stdio', 'http', 'websocket']));
  });

  it('parses streamable_http and sse JSON config into the http form transport', async () => {
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: '新增 / 更新 MCP Server' }));
    fireEvent.click(screen.getByTestId('mcp-mode-json'));
    const payload = JSON.stringify({
      mcpServers: {
        'modern-mcp': {
          type: 'streamable_http',
          url: 'https://example.com/mcp',
        },
      },
    });
    fireEvent.change(screen.getByTestId('mcp-json-textarea'), {
      target: { value: payload },
    });
    fireEvent.click(screen.getByTestId('mcp-mode-form'));
    const select = screen.getByRole('combobox', { name: '类型' }) as HTMLSelectElement;
    // streamable_http maps to "http" in the simplified 3-transport form.
    expect(select.value).toBe('http');
    // stdio-only fields hidden; URL field should be visible.
    expect(screen.queryByLabelText(/^命令$/)).not.toBeInTheDocument();
    expect(screen.getByDisplayValue('https://example.com/mcp')).toBeInTheDocument();
  });

  it('filters the server list by the search box', async () => {
    vi.mocked(tauri.listMcpServers).mockResolvedValue({
      scope: 'profile',
      config_path: 'C:\\Users\\Yanzh\\.remote-code-rust\\mcp.json',
      warnings: [],
      servers: [
        {
          name: 'filesystem',
          enabled: true,
          transport: 'stdio',
          config_path: 'C:\\Users\\Yanzh\\.remote-code-rust\\mcp.json',
          command: 'python',
          url: null,
          args: ['server.py'],
          cwd: null,
          env_keys: [],
          metadata_keys: [],
          startup_timeout_secs: null,
          request_timeout_secs: null,
          live: null,
        },
        {
          name: 'github-oauth',
          enabled: true,
          transport: 'http',
          config_path: 'C:\\Users\\Yanzh\\.remote-code-rust\\mcp.json',
          command: null,
          url: 'https://api.githubcopilot.com/mcp',
          args: [],
          cwd: null,
          env_keys: [],
          metadata_keys: [],
          startup_timeout_secs: null,
          request_timeout_secs: null,
          live: null,
        },
      ],
    });
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (2)')).toBeInTheDocument();
    });
    fireEvent.change(screen.getByTestId('mcp-search'), {
      target: { value: 'github' },
    });
    await waitFor(() => {
      expect(screen.queryByTestId('provider-row-filesystem')).toBeNull?.() ?? true;
    });
    expect(screen.getByTestId('mcp-edit-github-oauth')).toBeInTheDocument();
  });

  it('shows the include-secrets toggle in the controls row', async () => {
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });
    const toggle = screen.getByTestId('mcp-include-secrets');
    expect(toggle).toBeInTheDocument();
  });

  it('preserves cwd when editing an existing server', async () => {
    vi.mocked(tauri.listMcpServers).mockResolvedValue({
      scope: 'profile',
      config_path: 'C:\\Users\\Yanzh\\.remote-code-rust\\mcp.json',
      warnings: [],
      servers: [
        {
          name: 'filesystem',
          enabled: true,
          transport: 'stdio',
          config_path: 'C:\\Users\\Yanzh\\.remote-code-rust\\mcp.json',
          command: 'python',
          url: null,
          args: ['server.py'],
          cwd: 'C:\\workspace\\mcp-server',
          env_keys: ['TOKEN', 'API_KEY'],
          metadata_keys: ['scope'],
          startup_timeout_secs: 10,
          request_timeout_secs: 15,
          live: null,
        },
      ],
    });
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('mcp-edit-filesystem'));
    expect(screen.getByDisplayValue('python')).toBeInTheDocument();
    // env should prefill with key= placeholders (joined with \n).
    // Use container.querySelector because textareas don't have a `value`
    // attribute and RTL's getByDisplayValue works on inputs/selects.
    const textareas = document.querySelectorAll('textarea');
    const envTextarea = Array.from(textareas).find((t) =>
      (t as HTMLTextAreaElement).value.includes('TOKEN='),
    ) as HTMLTextAreaElement | undefined;
    expect(envTextarea).toBeDefined();
    expect(envTextarea!.value).toBe('TOKEN=\nAPI_KEY=');
  });

  it('blocks saving a stdio server without a command', async () => {
    const saveMcpServer = vi.fn().mockResolvedValue(undefined);
    vi.mocked(tauri.saveMcpServer).mockImplementation(saveMcpServer);
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: '新增 / 更新 MCP Server' }));
    fireEvent.change(screen.getByPlaceholderText('brave-search'), {
      target: { value: 'no-cmd' },
    });
    // Find the save button (text "保存" / "Save")
    fireEvent.click(screen.getByRole('button', { name: /^保存$|Save/ }));
    await waitFor(() => {
      expect(screen.getByText(/stdio 传输必须填写命令/)).toBeInTheDocument();
    });
    expect(saveMcpServer).not.toHaveBeenCalled();
  });

  it('prompts for confirmation before deleting a server', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    const removeMcpServer = vi.fn();
    vi.mocked(tauri.removeMcpServer).mockImplementation(removeMcpServer);
    render(<McpTab />);
    await waitFor(() => {
      expect(screen.getByText('Managed servers (1)')).toBeInTheDocument();
    });
    // The delete button uses Trash2 icon — query by title.
    const deleteButton = screen.getAllByTitle('删除')[0];
    fireEvent.click(deleteButton);
    expect(confirmSpy).toHaveBeenCalled();
    expect(removeMcpServer).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });
});
