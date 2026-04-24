import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { McpServerImportDialog } from './McpServerImportDialog';

afterEach(() => {
  cleanup();
});

describe('McpServerImportDialog', () => {
  const servers = {
    'my-server': { command: 'node', args: ['server.js'] },
    'other-server': { command: 'python', args: ['main.py'] },
  };

  it('renders with data-testid', () => {
    render(<McpServerImportDialog servers={servers} scope="project" onDone={vi.fn()} />);
    expect(screen.getByTestId('mcp-server-import-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<McpServerImportDialog servers={servers} scope="project" onDone={vi.fn()} />);
    expect(screen.getByText('Import MCP Servers')).toBeInTheDocument();
  });

  it('shows server names', () => {
    render(<McpServerImportDialog servers={servers} scope="project" onDone={vi.fn()} />);
    expect(screen.getByText('my-server')).toBeInTheDocument();
    expect(screen.getByText('other-server')).toBeInTheDocument();
  });

  it('shows scope', () => {
    render(<McpServerImportDialog servers={servers} scope="user" onDone={vi.fn()} />);
    expect(screen.getByText('user')).toBeInTheDocument();
  });

  it('calls onDone when import is clicked', () => {
    const onDone = vi.fn();
    render(<McpServerImportDialog servers={servers} scope="project" onDone={onDone} />);
    fireEvent.click(screen.getByTestId('mcp-server-import-confirm'));
    expect(onDone).toHaveBeenCalledOnce();
  });

  it('calls onDone when cancel is clicked', () => {
    const onDone = vi.fn();
    render(<McpServerImportDialog servers={servers} scope="project" onDone={onDone} />);
    fireEvent.click(screen.getByTestId('mcp-server-import-cancel'));
    expect(onDone).toHaveBeenCalledOnce();
  });
});
