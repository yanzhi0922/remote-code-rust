import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { McpToolInfo } from '../../lib/types';
import { MCPToolDetailView } from './MCPToolDetailView';

afterEach(() => { cleanup(); });

function makeTool(overrides: Partial<McpToolInfo> = {}): McpToolInfo {
  return {
    name: 'read_file',
    description: '读取文件内容',
    inputSchema: { type: 'object', properties: { path: { type: 'string' } } },
    ...overrides,
  };
}

describe('MCPToolDetailView', () => {
  it('renders with data-testid', () => {
    render(<MCPToolDetailView tool={makeTool()} serverName="srv" onBack={vi.fn()} />);
    expect(screen.getByTestId('mcp-tool-detail-view')).toBeTruthy();
  });

  it('displays tool name and server name', () => {
    render(<MCPToolDetailView tool={makeTool()} serverName="my-server" onBack={vi.fn()} />);
    expect(screen.getByText('read_file')).toBeTruthy();
    expect(screen.getByText(/来自 my-server/)).toBeTruthy();
  });

  it('displays description', () => {
    render(<MCPToolDetailView tool={makeTool()} serverName="srv" onBack={vi.fn()} />);
    expect(screen.getByText('读取文件内容')).toBeTruthy();
  });

  it('hides description when not provided', () => {
    render(<MCPToolDetailView tool={makeTool({ description: undefined })} serverName="srv" onBack={vi.fn()} />);
    expect(screen.queryByText('描述')).toBeNull();
  });

  it('displays input schema as JSON', () => {
    render(<MCPToolDetailView tool={makeTool()} serverName="srv" onBack={vi.fn()} />);
    expect(screen.getByText('输入 Schema')).toBeTruthy();
  });

  it('handles string inputSchema', () => {
    render(
      <MCPToolDetailView
        tool={makeTool({ inputSchema: '{"type":"string"}' })}
        serverName="srv"
        onBack={vi.fn()}
      />,
    );
    expect(screen.getByText('输入 Schema')).toBeTruthy();
  });

  it('hides schema section when inputSchema is null', () => {
    render(
      <MCPToolDetailView
        tool={makeTool({ inputSchema: undefined })}
        serverName="srv"
        onBack={vi.fn()}
      />,
    );
    expect(screen.queryByText('输入 Schema')).toBeNull();
  });

  it('calls onBack when back button clicked', () => {
    const onBack = vi.fn();
    render(<MCPToolDetailView tool={makeTool()} serverName="srv" onBack={onBack} />);
    fireEvent.click(screen.getByTestId('mcp-tool-detail-back'));
    expect(onBack).toHaveBeenCalledOnce();
  });

  it('applies custom className', () => {
    render(<MCPToolDetailView tool={makeTool()} serverName="srv" onBack={vi.fn()} className="mt-2" />);
    expect(screen.getByTestId('mcp-tool-detail-view').classList.contains('mt-2')).toBe(true);
  });

  it('shows copy button when schema exists', () => {
    render(<MCPToolDetailView tool={makeTool()} serverName="srv" onBack={vi.fn()} />);
    expect(screen.getByTestId('mcp-tool-detail-copy')).toBeTruthy();
  });
});
