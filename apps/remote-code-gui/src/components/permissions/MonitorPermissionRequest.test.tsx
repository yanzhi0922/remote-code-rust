import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { MonitorPermissionRequest } from './MonitorPermissionRequest';

const baseRequest: PermissionRequestInfo = {
  request_id: 'r1', tool_name: 'Monitor', tool_use_id: 't1',
  title: 'Monitor', description: 'Screen access', input: {},
  blocked_path: null, permission_suggestions: [],
};

describe('MonitorPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<MonitorPermissionRequest request={baseRequest} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('monitor-permission-request')).toBeInTheDocument();
  });

  it('calls onAllow', () => {
    const onAllow = vi.fn();
    render(<MonitorPermissionRequest request={baseRequest} onAllow={onAllow} onReject={vi.fn()} />);
    fireEvent.click(screen.getByText('允许执行'));
    expect(onAllow).toHaveBeenCalled();
  });

  it('calls onReject', () => {
    const onReject = vi.fn();
    render(<MonitorPermissionRequest request={baseRequest} onAllow={vi.fn()} onReject={onReject} />);
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalled();
  });
});
