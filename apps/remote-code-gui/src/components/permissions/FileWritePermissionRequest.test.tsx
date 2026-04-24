import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { FileWritePermissionRequest } from './FileWritePermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-write-1',
    tool_name: 'Write',
    tool_use_id: 'tool-write-1',
    title: 'File Write',
    description: 'Write a file',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('FileWritePermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <FileWritePermissionRequest
        request={makeRequest({ input: { file_path: '/src/new.ts', content: 'hello' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByTestId('file-write-permission-request')).toBeInTheDocument();
  });

  it('displays file path and content', () => {
    render(
      <FileWritePermissionRequest
        request={makeRequest({ input: { file_path: '/src/new.ts', content: 'hello world' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('/src/new.ts')).toBeInTheDocument();
    expect(screen.getByText('hello world')).toBeInTheDocument();
  });

  it('truncates content over 500 characters', () => {
    const longContent = 'a'.repeat(600);
    render(
      <FileWritePermissionRequest
        request={makeRequest({ input: { file_path: '/src/big.ts', content: longContent } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText(/内容已截断/)).toBeInTheDocument();
    expect(screen.getByText(/共 600 字符/)).toBeInTheDocument();
  });

  it('shows no content message when content is empty', () => {
    render(
      <FileWritePermissionRequest
        request={makeRequest({ input: { file_path: '/src/empty.ts' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('无文件内容')).toBeInTheDocument();
  });

  it('calls onAllow when allow button is clicked', () => {
    const onAllow = vi.fn();
    render(
      <FileWritePermissionRequest
        request={makeRequest({ input: { file_path: '/src/a.ts', content: 'x' } })}
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
      <FileWritePermissionRequest
        request={makeRequest({ input: { file_path: '/src/a.ts', content: 'x' } })}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });

  it('handles null input gracefully', () => {
    render(
      <FileWritePermissionRequest
        request={makeRequest({ input: null as unknown as Record<string, unknown> })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('无文件内容')).toBeInTheDocument();
  });
});
