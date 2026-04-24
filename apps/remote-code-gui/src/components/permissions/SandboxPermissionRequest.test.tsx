import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { SandboxPermissionRequest } from './SandboxPermissionRequest';

const base: PermissionRequestInfo = {
  request_id: 'r1', tool_name: 'Sandbox', tool_use_id: 't1',
  title: 'Sandbox', description: 'Sandbox exec', input: {},
  blocked_path: null, permission_suggestions: [],
};

describe('SandboxPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<SandboxPermissionRequest request={base} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('sandbox-permission-request')).toBeInTheDocument();
  });

  it('calls onAllow', () => {
    const fn = vi.fn();
    render(<SandboxPermissionRequest request={base} onAllow={fn} onReject={vi.fn()} />);
    fireEvent.click(screen.getByText('允许执行'));
    expect(fn).toHaveBeenCalled();
  });

  it('calls onReject', () => {
    const fn = vi.fn();
    render(<SandboxPermissionRequest request={base} onAllow={vi.fn()} onReject={fn} />);
    fireEvent.click(screen.getByText('拒绝'));
    expect(fn).toHaveBeenCalled();
  });
});
