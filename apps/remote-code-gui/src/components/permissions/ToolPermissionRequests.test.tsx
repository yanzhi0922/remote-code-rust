import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import {
  BashPermissionRequest,
  FileEditPermissionRequest,
  FileWritePermissionRequest,
  McpPermissionRequest,
  GenericPermissionRequest,
} from './ToolPermissionRequests';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-1',
    tool_name: 'Bash',
    tool_use_id: 'tool-1',
    title: 'Test Permission',
    description: 'A test permission request',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('BashPermissionRequest', () => {
  afterEach(cleanup);

  it('renders command content', () => {
    render(
      <BashPermissionRequest
        request={makeRequest({ input: { command: 'ls -la' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('ls -la')).toBeInTheDocument();
  });

  it('highlights dangerous commands', () => {
    render(
      <BashPermissionRequest
        request={makeRequest({ input: { command: 'rm -rf /' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('⚠ 危险命令')).toBeInTheDocument();
  });

  it('calls onAllow when allow button is clicked', () => {
    const onAllow = vi.fn();
    render(
      <BashPermissionRequest
        request={makeRequest({ input: { command: 'ls' } })}
        onAllow={onAllow}
        onReject={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByText('允许执行'));
    expect(onAllow).toHaveBeenCalledTimes(1);
  });

  it('calls onReject when reject button is clicked', () => {
    const onReject = vi.fn();
    render(
      <BashPermissionRequest
        request={makeRequest({ input: { command: 'ls' } })}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });
});

describe('FileEditPermissionRequest', () => {
  afterEach(cleanup);

  it('renders file path and edit content', () => {
    render(
      <FileEditPermissionRequest
        request={makeRequest({
          tool_name: 'Edit',
          input: { file_path: '/src/index.ts', old_text: 'foo', new_text: 'bar' },
        })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('/src/index.ts')).toBeInTheDocument();
    expect(screen.getByText('foo')).toBeInTheDocument();
    expect(screen.getByText('bar')).toBeInTheDocument();
  });

  it('shows old and new content labels', () => {
    render(
      <FileEditPermissionRequest
        request={makeRequest({
          tool_name: 'Edit',
          input: { file_path: '/src/a.ts', old_text: 'old', new_text: 'new' },
        })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('- 旧内容')).toBeInTheDocument();
    expect(screen.getByText('+ 新内容')).toBeInTheDocument();
  });

  it('calls onAllow', () => {
    const onAllow = vi.fn();
    render(
      <FileEditPermissionRequest
        request={makeRequest({ tool_name: 'Edit', input: {} })}
        onAllow={onAllow}
        onReject={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByText('允许执行'));
    expect(onAllow).toHaveBeenCalled();
  });
});

describe('FileWritePermissionRequest', () => {
  afterEach(cleanup);

  it('renders file path and content', () => {
    render(
      <FileWritePermissionRequest
        request={makeRequest({
          tool_name: 'Write',
          input: { file_path: '/src/new.ts', content: 'hello world' },
        })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('/src/new.ts')).toBeInTheDocument();
    expect(screen.getByText('hello world')).toBeInTheDocument();
  });

  it('calls onReject', () => {
    const onReject = vi.fn();
    render(
      <FileWritePermissionRequest
        request={makeRequest({ tool_name: 'Write', input: {} })}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalled();
  });
});

describe('McpPermissionRequest', () => {
  afterEach(cleanup);

  it('renders MCP tool name and arguments', () => {
    render(
      <McpPermissionRequest
        request={makeRequest({
          tool_name: 'mcp_tool',
          input: { tool_name: 'search', arguments: { query: 'test' } },
        })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText(/search/)).toBeInTheDocument();
  });

  it('calls onAllow', () => {
    const onAllow = vi.fn();
    render(
      <McpPermissionRequest
        request={makeRequest({ tool_name: 'mcp', input: {} })}
        onAllow={onAllow}
        onReject={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByText('允许执行'));
    expect(onAllow).toHaveBeenCalled();
  });
});

describe('GenericPermissionRequest', () => {
  afterEach(cleanup);

  it('renders tool name and formatted input', () => {
    render(
      <GenericPermissionRequest
        request={makeRequest({
          tool_name: 'CustomTool',
          title: 'Custom Tool Request',
          input: { key: 'value' },
        })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('Custom Tool Request')).toBeInTheDocument();
    expect(screen.getByText(/"key"/)).toBeInTheDocument();
  });

  it('calls onAllow and onReject', () => {
    const onAllow = vi.fn();
    const onReject = vi.fn();
    render(
      <GenericPermissionRequest
        request={makeRequest({ input: {} })}
        onAllow={onAllow}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('允许执行'));
    expect(onAllow).toHaveBeenCalled();
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalled();
  });
});
