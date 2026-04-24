import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { McpToolInfo } from '../../lib/types';
import { McpToolList } from './McpToolList';

const tools: McpToolInfo[] = [
  { name: 'read_file', description: 'Read the contents of a file from the filesystem' },
  { name: 'write_file', description: 'Write content to a file on the filesystem' },
  { name: 'list_directory', description: 'List all files and directories in a given path. This is a longer description that should be truncated when displayed in the tool list component.' },
];

describe('McpToolList', () => {
  beforeEach(() => cleanup());
  afterEach(() => cleanup());

  it('renders all tool names', () => {
    render(<McpToolList tools={tools} onSelectTool={vi.fn()} serverName="filesystem" />);
    expect(screen.getByText('read_file')).toBeInTheDocument();
    expect(screen.getByText('write_file')).toBeInTheDocument();
    expect(screen.getByText('list_directory')).toBeInTheDocument();
  });

  it('shows empty state when no tools', () => {
    render(<McpToolList tools={[]} onSelectTool={vi.fn()} serverName="empty" />);
    expect(screen.getByText('该服务器没有可用工具')).toBeInTheDocument();
  });

  it('calls onSelectTool when a tool is clicked', () => {
    const onSelectTool = vi.fn();
    render(<McpToolList tools={tools} onSelectTool={onSelectTool} serverName="filesystem" />);
    fireEvent.click(screen.getByTestId('mcp-tool-item-read_file'));
    expect(onSelectTool).toHaveBeenCalledWith('read_file');
  });

  it('filters tools by name', () => {
    render(<McpToolList tools={tools} onSelectTool={vi.fn()} serverName="filesystem" />);
    fireEvent.change(screen.getByTestId('mcp-tool-search'), { target: { value: 'read' } });
    expect(screen.getByText('read_file')).toBeInTheDocument();
    expect(screen.queryByText('write_file')).not.toBeInTheDocument();
  });

  it('truncates long descriptions to 80 characters', () => {
    render(<McpToolList tools={tools} onSelectTool={vi.fn()} serverName="filesystem" />);
    const longDesc = tools[2].description!;
    const truncatedEl = screen.getByTestId('mcp-tool-item-list_directory');
    expect(truncatedEl.textContent).toContain('...');
    expect(truncatedEl.textContent!.length).toBeLessThan(longDesc.length + 100);
  });

  it('shows server name in header', () => {
    render(<McpToolList tools={tools} onSelectTool={vi.fn()} serverName="my-server" />);
    expect(screen.getByText(/my-server 的工具/)).toBeInTheDocument();
  });

  it('shows tool count badge', () => {
    render(<McpToolList tools={tools} onSelectTool={vi.fn()} serverName="filesystem" />);
    expect(screen.getByText('3')).toBeInTheDocument();
  });
});
