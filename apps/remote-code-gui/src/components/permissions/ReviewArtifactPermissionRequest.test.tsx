import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { ReviewArtifactPermissionRequest } from './ReviewArtifactPermissionRequest';

const base: PermissionRequestInfo = {
  request_id: 'r1', tool_name: 'Review', tool_use_id: 't1',
  title: 'Review', description: 'Review artifact', input: {},
  blocked_path: null, permission_suggestions: [],
};

describe('ReviewArtifactPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<ReviewArtifactPermissionRequest request={base} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('review-artifact-permission')).toBeInTheDocument();
  });

  it('calls onAllow', () => {
    const fn = vi.fn();
    render(<ReviewArtifactPermissionRequest request={base} onAllow={fn} onReject={vi.fn()} />);
    fireEvent.click(screen.getByText('允许执行'));
    expect(fn).toHaveBeenCalled();
  });

  it('calls onReject', () => {
    const fn = vi.fn();
    render(<ReviewArtifactPermissionRequest request={base} onAllow={vi.fn()} onReject={fn} />);
    fireEvent.click(screen.getByText('拒绝'));
    expect(fn).toHaveBeenCalled();
  });
});
