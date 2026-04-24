import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { McpReconnect } from './McpReconnect';

describe('McpReconnect', () => {
  beforeEach(() => cleanup());
  afterEach(() => cleanup());

  it('shows spinner and reconnecting message when reconnecting', () => {
    render(<McpReconnect serverName="my-server" reconnecting={true} onReconnect={vi.fn()} />);
    expect(screen.getByText(/正在重连/)).toBeInTheDocument();
    expect(screen.getByText('my-server')).toBeInTheDocument();
  });

  it('shows error message when error is provided', () => {
    render(<McpReconnect serverName="my-server" reconnecting={false} onReconnect={vi.fn()} error="Connection refused" />);
    expect(screen.getByText('Connection refused')).toBeInTheDocument();
  });

  it('shows retry button when there is an error', () => {
    render(<McpReconnect serverName="my-server" reconnecting={false} onReconnect={vi.fn()} error="fail" />);
    expect(screen.getByTestId('mcp-reconnect-retry')).toBeInTheDocument();
  });

  it('calls onReconnect when retry button clicked', () => {
    const onReconnect = vi.fn();
    render(<McpReconnect serverName="my-server" reconnecting={false} onReconnect={onReconnect} error="fail" />);
    fireEvent.click(screen.getByTestId('mcp-reconnect-retry'));
    expect(onReconnect).toHaveBeenCalledTimes(1);
  });

  it('shows error state when not reconnecting and no initial reconnecting', () => {
    render(<McpReconnect serverName="my-server" reconnecting={false} onReconnect={vi.fn()} error="timeout" />);
    expect(screen.getByText('timeout')).toBeInTheDocument();
    expect(screen.getByTestId('mcp-reconnect-retry')).toBeInTheDocument();
  });

  it('renders without error when error is null', () => {
    render(<McpReconnect serverName="my-server" reconnecting={false} onReconnect={vi.fn()} error={null} />);
    expect(screen.getByTestId('mcp-reconnect')).toBeInTheDocument();
  });
});
