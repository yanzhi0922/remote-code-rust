import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { FallbackPermissionRequest } from './FallbackPermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-1',
    tool_name: 'Unknown',
    tool_use_id: 'tool-1',
    title: 'Fallback',
    description: 'Unknown tool',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('FallbackPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<FallbackPermissionRequest request={makeRequest()} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('fallback-permission-request')).toBeInTheDocument();
  });

  it('calls onAllow', () => {
    const onAllow = vi.fn();
    render(<FallbackPermissionRequest request={makeRequest()} onAllow={onAllow} onReject={vi.fn()} />);
    fireEvent.click(screen.getByText('允许执行'));
    expect(onAllow).toHaveBeenCalledTimes(1);
  });

  it('calls onReject', () => {
    const onReject = vi.fn();
    render(<FallbackPermissionRequest request={makeRequest()} onAllow={vi.fn()} onReject={onReject} />);
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });
});
