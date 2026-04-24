import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { SedEditPermissionRequest } from './SedEditPermissionRequest';

const base: PermissionRequestInfo = {
  request_id: 'r1', tool_name: 'SedEdit', tool_use_id: 't1',
  title: 'Sed', description: 'Sed edit', input: {},
  blocked_path: null, permission_suggestions: [],
};

describe('SedEditPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<SedEditPermissionRequest request={base} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('sed-edit-permission')).toBeInTheDocument();
  });

  it('calls onAllow', () => {
    const fn = vi.fn();
    render(<SedEditPermissionRequest request={base} onAllow={fn} onReject={vi.fn()} />);
    fireEvent.click(screen.getByText('允许执行'));
    expect(fn).toHaveBeenCalled();
  });
});
