import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { FilePermissionDialog } from './FilePermissionDialog';

function makeRequest(input: Record<string, unknown> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-1',
    tool_name: 'FileEdit',
    tool_use_id: 'tool-1',
    title: 'File Edit',
    description: 'Edit a file',
    input,
    blocked_path: null,
    permission_suggestions: [],
  };
}

describe('FilePermissionDialog', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<FilePermissionDialog request={makeRequest()} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('file-permission-dialog')).toBeInTheDocument();
  });

  it('shows file path', () => {
    render(<FilePermissionDialog request={makeRequest({ path: '/src/app.ts' })} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByText('/src/app.ts')).toBeInTheDocument();
  });

  it('calls onAllow', () => {
    const onAllow = vi.fn();
    render(<FilePermissionDialog request={makeRequest()} onAllow={onAllow} onReject={vi.fn()} />);
    fireEvent.click(screen.getByText('允许执行'));
    expect(onAllow).toHaveBeenCalled();
  });

  it('calls onReject', () => {
    const onReject = vi.fn();
    render(<FilePermissionDialog request={makeRequest()} onAllow={vi.fn()} onReject={onReject} />);
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalled();
  });
});
