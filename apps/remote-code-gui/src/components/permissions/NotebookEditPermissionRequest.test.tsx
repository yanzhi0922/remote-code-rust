import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { NotebookEditPermissionRequest } from './NotebookEditPermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-nb-1',
    tool_name: 'NotebookEdit',
    tool_use_id: 'tool-nb-1',
    title: 'Notebook Edit',
    description: 'Edit a notebook cell',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('NotebookEditPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <NotebookEditPermissionRequest
        request={makeRequest({ input: { notebook_path: '/notebooks/test.ipynb', cell_number: 3 } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByTestId('notebook-edit-permission-request')).toBeInTheDocument();
  });

  it('displays notebook path', () => {
    render(
      <NotebookEditPermissionRequest
        request={makeRequest({ input: { notebook_path: '/notebooks/analysis.ipynb' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('/notebooks/analysis.ipynb')).toBeInTheDocument();
  });

  it('displays cell number', () => {
    render(
      <NotebookEditPermissionRequest
        request={makeRequest({ input: { notebook_path: '/nb.ipynb', cell_number: 5 } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('#5')).toBeInTheDocument();
  });

  it('shows no info message when fields are missing', () => {
    render(
      <NotebookEditPermissionRequest
        request={makeRequest({ input: {} })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('无 Notebook 信息')).toBeInTheDocument();
  });

  it('handles string cell_number', () => {
    render(
      <NotebookEditPermissionRequest
        request={makeRequest({ input: { notebook_path: '/nb.ipynb', cell_number: '10' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('#10')).toBeInTheDocument();
  });

  it('calls onAllow when allow button is clicked', () => {
    const onAllow = vi.fn();
    render(
      <NotebookEditPermissionRequest
        request={makeRequest({ input: { notebook_path: '/nb.ipynb', cell_number: 1 } })}
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
      <NotebookEditPermissionRequest
        request={makeRequest({ input: { notebook_path: '/nb.ipynb' } })}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });
});
