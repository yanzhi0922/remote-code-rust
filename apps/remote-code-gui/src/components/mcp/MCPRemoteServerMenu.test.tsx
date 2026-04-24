import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MCPRemoteServerMenu } from './MCPRemoteServerMenu';

describe('MCPRemoteServerMenu', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<MCPRemoteServerMenu serverName="s" url="http://x" />);
    expect(screen.getByTestId('mcp-remote-server-menu')).toBeInTheDocument();
  });

  it('shows server name and url', () => {
    render(<MCPRemoteServerMenu serverName="remote" url="https://api.example.com" />);
    expect(screen.getByText('remote')).toBeInTheDocument();
    expect(screen.getByText('https://api.example.com')).toBeInTheDocument();
  });

  it('calls onRemove', () => {
    const fn = vi.fn();
    render(<MCPRemoteServerMenu serverName="s" url="u" onRemove={fn} />);
    fireEvent.click(screen.getByTitle('菜单'));
    fireEvent.click(screen.getByText('删除'));
    expect(fn).toHaveBeenCalled();
  });
});
