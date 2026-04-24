import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { McpToolInfo } from '../../lib/types';
import { MCPToolListView } from './MCPToolListView';

afterEach(() => { cleanup(); });

function makeTools(): McpToolInfo[] {
  return [
    { name: 'read_file', description: '读取文件内容' },
    { name: 'write_file', description: '写入文件内容' },
    { name: 'list_dir', description: '列出目录内容' },
  ];
}

describe('MCPToolListView', () => {
  it('renders with data-testid', () => {
    render(<MCPToolListView tools={makeTools()} serverName="srv" onSelectTool={vi.fn()} />);
    expect(screen.getByTestId('mcp-tool-list-view')).toBeTruthy();
  });

  it('displays server name and tool count', () => {
    render(<MCPToolListView tools={makeTools()} serverName="my-server" onSelectTool={vi.fn()} />);
    expect(screen.getByText(/my-server 的工具/)).toBeTruthy();
    expect(screen.getByText('3')).toBeTruthy();
  });

  it('renders all tools', () => {
    render(<MCPToolListView tools={makeTools()} serverName="srv" onSelectTool={vi.fn()} />);
    expect(screen.getByTestId('mcp-tool-list-item-read_file')).toBeTruthy();
    expect(screen.getByTestId('mcp-tool-list-item-write_file')).toBeTruthy();
    expect(screen.getByTestId('mcp-tool-list-item-list_dir')).toBeTruthy();
  });

  it('displays tool descriptions', () => {
    render(<MCPToolListView tools={makeTools()} serverName="srv" onSelectTool={vi.fn()} />);
    expect(screen.getByText('读取文件内容')).toBeTruthy();
  });

  it('calls onSelectTool when tool clicked', () => {
    const onSelect = vi.fn();
    render(<MCPToolListView tools={makeTools()} serverName="srv" onSelectTool={onSelect} />);
    fireEvent.click(screen.getByTestId('mcp-tool-list-item-read_file'));
    expect(onSelect).toHaveBeenCalledWith('read_file');
  });

  it('filters tools by search query', () => {
    render(<MCPToolListView tools={makeTools()} serverName="srv" onSelectTool={vi.fn()} />);
    fireEvent.change(screen.getByTestId('mcp-tool-list-search'), { target: { value: 'read' } });
    expect(screen.getByTestId('mcp-tool-list-item-read_file')).toBeTruthy();
    expect(screen.queryByTestId('mcp-tool-list-item-write_file')).toBeNull();
  });

  it('shows empty message when no tools match search', () => {
    render(<MCPToolListView tools={makeTools()} serverName="srv" onSelectTool={vi.fn()} />);
    fireEvent.change(screen.getByTestId('mcp-tool-list-search'), { target: { value: 'xyz' } });
    expect(screen.getByText('没有匹配的工具')).toBeTruthy();
  });

  it('shows empty message when tools array is empty', () => {
    render(<MCPToolListView tools={[]} serverName="srv" onSelectTool={vi.fn()} />);
    expect(screen.getByText('该服务器没有可用工具')).toBeTruthy();
  });

  it('applies custom className', () => {
    render(<MCPToolListView tools={makeTools()} serverName="srv" onSelectTool={vi.fn()} className="p-4" />);
    expect(screen.getByTestId('mcp-tool-list-view').classList.contains('p-4')).toBe(true);
  });

  it('truncates long descriptions', () => {
    const longDesc = 'a'.repeat(100);
    const tools: McpToolInfo[] = [{ name: 'tool', description: longDesc }];
    render(<MCPToolListView tools={tools} serverName="srv" onSelectTool={vi.fn()} />);
    const desc = screen.getByTestId('mcp-tool-list-item-tool').querySelector('.text-xs');
    expect(desc?.textContent?.endsWith('...')).toBe(true);
  });
});
