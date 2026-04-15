import { cleanup, render, screen, waitFor } from '@testing-library/react';
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

    expect(screen.getByText('filesystem')).toBeInTheDocument();
    expect(
      await screen.findByText(/无法加载 runtime inventory：runtime discovery failed/),
    ).toBeInTheDocument();
  });
});
