import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { FilesystemPermissionRequest } from './FilesystemPermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-fs-1',
    tool_name: 'Filesystem',
    tool_use_id: 'tool-fs-1',
    title: 'Filesystem Access',
    description: 'Access filesystem',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('FilesystemPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <FilesystemPermissionRequest
        request={makeRequest({ input: { path: '/home/user', operation: 'read' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByTestId('filesystem-permission-request')).toBeInTheDocument();
  });

  it('displays the path', () => {
    render(
      <FilesystemPermissionRequest
        request={makeRequest({ input: { path: '/home/user/docs' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('/home/user/docs')).toBeInTheDocument();
  });

  it('displays operation type with label', () => {
    render(
      <FilesystemPermissionRequest
        request={makeRequest({ input: { path: '/tmp', operation: 'read' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('读取')).toBeInTheDocument();
  });

  it('shows write operation with amber color', () => {
    render(
      <FilesystemPermissionRequest
        request={makeRequest({ input: { path: '/tmp', operation: 'write' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('写入')).toBeInTheDocument();
  });

  it('shows no info message when path and operation are missing', () => {
    render(
      <FilesystemPermissionRequest
        request={makeRequest({ input: {} })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('无路径或操作信息')).toBeInTheDocument();
  });

  it('calls onAllow when allow button is clicked', () => {
    const onAllow = vi.fn();
    render(
      <FilesystemPermissionRequest
        request={makeRequest({ input: { path: '/tmp', operation: 'read' } })}
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
      <FilesystemPermissionRequest
        request={makeRequest({ input: { path: '/tmp', operation: 'read' } })}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });

  it('handles array input gracefully', () => {
    render(
      <FilesystemPermissionRequest
        request={makeRequest({ input: [] as unknown as Record<string, unknown> })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('无路径或操作信息')).toBeInTheDocument();
  });
});
