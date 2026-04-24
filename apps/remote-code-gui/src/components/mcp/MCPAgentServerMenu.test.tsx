import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MCPAgentServerMenu } from './MCPAgentServerMenu';

describe('MCPAgentServerMenu', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<MCPAgentServerMenu serverName="test" agentName="agent" />);
    expect(screen.getByTestId('mcp-agent-server-menu')).toBeInTheDocument();
  });

  it('shows server name', () => {
    render(<MCPAgentServerMenu serverName="my-server" agentName="a" />);
    expect(screen.getByText('my-server')).toBeInTheDocument();
  });

  it('shows connect button when onConnect provided', () => {
    render(<MCPAgentServerMenu serverName="s" agentName="a" onConnect={vi.fn()} />);
    fireEvent.click(screen.getByTitle('菜单'));
    expect(screen.getByText('连接')).toBeInTheDocument();
  });

  it('calls onConnect', () => {
    const fn = vi.fn();
    render(<MCPAgentServerMenu serverName="s" agentName="a" onConnect={fn} />);
    fireEvent.click(screen.getByTitle('菜单'));
    fireEvent.click(screen.getByText('连接'));
    expect(fn).toHaveBeenCalled();
  });
});
