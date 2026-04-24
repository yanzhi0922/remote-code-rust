import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { McpServerApprovalDialog } from './McpServerApprovalDialog';

afterEach(() => {
  cleanup();
});

describe('McpServerApprovalDialog', () => {
  it('renders with data-testid', () => {
    render(<McpServerApprovalDialog serverName="my-server" onDone={vi.fn()} />);
    expect(screen.getByTestId('mcp-server-approval-dialog')).toBeInTheDocument();
  });

  it('shows server name', () => {
    render(<McpServerApprovalDialog serverName="my-server" onDone={vi.fn()} />);
    expect(screen.getByText('my-server')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<McpServerApprovalDialog serverName="my-server" onDone={vi.fn()} />);
    expect(screen.getByText('New MCP Server Found')).toBeInTheDocument();
  });

  it('calls onDone with yes_all when approve all is clicked', () => {
    const onDone = vi.fn();
    render(<McpServerApprovalDialog serverName="my-server" onDone={onDone} />);
    fireEvent.click(screen.getByTestId('mcp-server-approval-yes-all'));
    expect(onDone).toHaveBeenCalledWith('yes_all');
  });

  it('calls onDone with yes when approve is clicked', () => {
    const onDone = vi.fn();
    render(<McpServerApprovalDialog serverName="my-server" onDone={onDone} />);
    fireEvent.click(screen.getByTestId('mcp-server-approval-yes'));
    expect(onDone).toHaveBeenCalledWith('yes');
  });

  it('calls onDone with no when reject is clicked', () => {
    const onDone = vi.fn();
    render(<McpServerApprovalDialog serverName="my-server" onDone={onDone} />);
    fireEvent.click(screen.getByTestId('mcp-server-approval-no'));
    expect(onDone).toHaveBeenCalledWith('no');
  });
});
