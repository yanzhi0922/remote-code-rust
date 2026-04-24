import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MCPListPanel } from './MCPListPanel';
import type { McpServerInfo } from '../../lib/types';

function makeServer(name: string): McpServerInfo {
  return {
    name, enabled: true, transport: 'stdio', config_path: '/test',
    command: null, url: null, args: [], cwd: null, env_keys: [],
    metadata_keys: [], startup_timeout_secs: null, request_timeout_secs: null,
    live: null,
  };
}

describe('MCPListPanel', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<MCPListPanel servers={[]} />);
    expect(screen.getByTestId('mcp-list-panel')).toBeInTheDocument();
  });

  it('shows servers', () => {
    render(<MCPListPanel servers={[makeServer('server-a')]} />);
    expect(screen.getByText('server-a')).toBeInTheDocument();
  });

  it('calls onSelect', () => {
    const fn = vi.fn();
    const server = makeServer('s1');
    render(<MCPListPanel servers={[server]} onSelect={fn} />);
    screen.getByTestId('mcp-list-item-s1').click();
    expect(fn).toHaveBeenCalledWith(server);
  });
});
