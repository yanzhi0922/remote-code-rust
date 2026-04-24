import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { PermissionDialog } from './PermissionDialog';

const baseRequest: PermissionRequestInfo = {
  request_id: 'r1', tool_name: 'Test', tool_use_id: 't1',
  title: 'Test Permission', description: 'A test', input: {},
  blocked_path: null, permission_suggestions: [],
};

describe('PermissionDialog', () => {
  afterEach(cleanup);

  it('renders when request is provided', () => {
    render(<PermissionDialog request={baseRequest} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('permission-dialog')).toBeInTheDocument();
  });

  it('returns null when no request', () => {
    const { container } = render(<PermissionDialog request={null} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(container.firstChild).toBeNull();
  });

  it('calls onAllow', () => {
    const onAllow = vi.fn();
    render(<PermissionDialog request={baseRequest} onAllow={onAllow} onReject={vi.fn()} />);
    fireEvent.click(screen.getByText('允许执行'));
    expect(onAllow).toHaveBeenCalled();
  });

  it('calls onReject', () => {
    const onReject = vi.fn();
    render(<PermissionDialog request={baseRequest} onAllow={vi.fn()} onReject={onReject} />);
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalled();
  });
});
