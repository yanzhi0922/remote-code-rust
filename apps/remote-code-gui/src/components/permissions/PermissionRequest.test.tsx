import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { PermissionRequest } from './PermissionRequest';

const base: PermissionRequestInfo = {
  request_id: 'r1', tool_name: 'Tool', tool_use_id: 't1',
  title: 'Test', description: 'Desc', input: {},
  blocked_path: null, permission_suggestions: [],
};

describe('PermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<PermissionRequest request={base} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('permission-request')).toBeInTheDocument();
  });

  it('calls onAllow', () => {
    const fn = vi.fn();
    render(<PermissionRequest request={base} onAllow={fn} onReject={vi.fn()} />);
    fireEvent.click(screen.getByText('允许执行'));
    expect(fn).toHaveBeenCalled();
  });

  it('calls onReject', () => {
    const fn = vi.fn();
    render(<PermissionRequest request={base} onAllow={vi.fn()} onReject={fn} />);
    fireEvent.click(screen.getByText('拒绝'));
    expect(fn).toHaveBeenCalled();
  });
});
