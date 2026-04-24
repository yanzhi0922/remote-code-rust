import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { McpToolInfo } from '../../lib/types';
import { McpToolDetail } from './McpToolDetail';

const baseTool: McpToolInfo = {
  name: 'read_file',
  description: 'Read the contents of a file from the filesystem',
};

const toolWithSchema: McpToolInfo & { inputSchema: unknown } = {
  name: 'write_file',
  description: 'Write content to a file',
  inputSchema: {
    type: 'object',
    properties: {
      path: { type: 'string', description: 'File path' },
      content: { type: 'string', description: 'File content' },
    },
    required: ['path', 'content'],
  },
};

describe('McpToolDetail', () => {
  beforeEach(() => cleanup());
  afterEach(() => cleanup());

  it('renders tool name and description', () => {
    render(<McpToolDetail tool={baseTool} serverName="filesystem" onBack={vi.fn()} />);
    expect(screen.getByText('read_file')).toBeInTheDocument();
    expect(screen.getByText('Read the contents of a file from the filesystem')).toBeInTheDocument();
  });

  it('shows server name', () => {
    render(<McpToolDetail tool={baseTool} serverName="my-server" onBack={vi.fn()} />);
    expect(screen.getByText(/来自 my-server/)).toBeInTheDocument();
  });

  it('calls onBack when back button clicked', () => {
    const onBack = vi.fn();
    render(<McpToolDetail tool={baseTool} serverName="filesystem" onBack={onBack} />);
    fireEvent.click(screen.getByTestId('mcp-tool-back-btn'));
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it('displays input schema when available', () => {
    render(<McpToolDetail tool={toolWithSchema} serverName="filesystem" onBack={vi.fn()} />);
    expect(screen.getByText('输入 Schema')).toBeInTheDocument();
    expect(screen.getByText(/"type": "object"/)).toBeInTheDocument();
  });

  it('does not display schema section when not available', () => {
    render(<McpToolDetail tool={baseTool} serverName="filesystem" onBack={vi.fn()} />);
    expect(screen.queryByText('输入 Schema')).not.toBeInTheDocument();
  });

  it('renders without description when null', () => {
    const noDescTool: McpToolInfo = { name: 'no_desc', description: null };
    render(<McpToolDetail tool={noDescTool} serverName="filesystem" onBack={vi.fn()} />);
    expect(screen.getByText('no_desc')).toBeInTheDocument();
    expect(screen.queryByText('描述')).not.toBeInTheDocument();
  });
});
