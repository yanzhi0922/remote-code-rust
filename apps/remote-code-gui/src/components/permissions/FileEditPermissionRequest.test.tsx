import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { FileEditPermissionRequest } from './FileEditPermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-edit-1',
    tool_name: 'Edit',
    tool_use_id: 'tool-edit-1',
    title: 'File Edit',
    description: 'Edit a file',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('FileEditPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <FileEditPermissionRequest
        request={makeRequest({ input: { file_path: '/src/a.ts', old_string: 'old', new_string: 'new' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByTestId('file-edit-permission-request')).toBeInTheDocument();
  });

  it('displays file path', () => {
    render(
      <FileEditPermissionRequest
        request={makeRequest({ input: { file_path: '/src/index.ts' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('/src/index.ts')).toBeInTheDocument();
  });

  it('shows diff with old and new content', () => {
    render(
      <FileEditPermissionRequest
        request={makeRequest({ input: { file_path: '/src/a.ts', old_string: 'foo', new_string: 'bar' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('foo')).toBeInTheDocument();
    expect(screen.getByText('bar')).toBeInTheDocument();
    expect(screen.getByText('- 删除内容')).toBeInTheDocument();
    expect(screen.getByText('+ 新增内容')).toBeInTheDocument();
  });

  it('hides diff when showDiff is false', () => {
    render(
      <FileEditPermissionRequest
        request={makeRequest({ input: { file_path: '/src/a.ts', old_string: 'foo', new_string: 'bar' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
        showDiff={false}
      />,
    );
    expect(screen.queryByText('foo')).not.toBeInTheDocument();
    expect(screen.queryByText('bar')).not.toBeInTheDocument();
  });

  it('shows no diff content message when both old and new are empty', () => {
    render(
      <FileEditPermissionRequest
        request={makeRequest({ input: { file_path: '/src/a.ts' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('无 diff 内容')).toBeInTheDocument();
  });

  it('calls onAllow when allow button is clicked', () => {
    const onAllow = vi.fn();
    render(
      <FileEditPermissionRequest
        request={makeRequest({ input: { file_path: '/src/a.ts' } })}
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
      <FileEditPermissionRequest
        request={makeRequest({ input: { file_path: '/src/a.ts' } })}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });

  it('extracts old_text and new_text as fallback keys', () => {
    render(
      <FileEditPermissionRequest
        request={makeRequest({ input: { file_path: '/src/b.ts', old_text: 'oldVal', new_text: 'newVal' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('oldVal')).toBeInTheDocument();
    expect(screen.getByText('newVal')).toBeInTheDocument();
  });
});
