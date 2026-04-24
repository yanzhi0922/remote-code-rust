import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { WebFetchPermissionRequest } from './WebFetchPermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-web-1',
    tool_name: 'WebFetch',
    tool_use_id: 'tool-web-1',
    title: 'Web Fetch',
    description: 'Fetch a web resource',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('WebFetchPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <WebFetchPermissionRequest
        request={makeRequest({ input: { url: 'https://example.com' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByTestId('web-fetch-permission-request')).toBeInTheDocument();
  });

  it('displays the URL as a link', () => {
    render(
      <WebFetchPermissionRequest
        request={makeRequest({ input: { url: 'https://example.com/api' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    const link = screen.getByText('https://example.com/api');
    expect(link).toBeInTheDocument();
    expect(link.tagName).toBe('A');
    expect(link.getAttribute('href')).toBe('https://example.com/api');
  });

  it('shows insecure warning for non-HTTPS URLs', () => {
    render(
      <WebFetchPermissionRequest
        request={makeRequest({ input: { url: 'http://example.com' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText(/非 HTTPS/)).toBeInTheDocument();
  });

  it('does not show insecure warning for HTTPS URLs', () => {
    render(
      <WebFetchPermissionRequest
        request={makeRequest({ input: { url: 'https://example.com' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.queryByText(/非 HTTPS/)).not.toBeInTheDocument();
  });

  it('shows no URL message when url is missing', () => {
    render(
      <WebFetchPermissionRequest
        request={makeRequest({ input: {} })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('无 URL 信息')).toBeInTheDocument();
  });

  it('calls onAllow when allow button is clicked', () => {
    const onAllow = vi.fn();
    render(
      <WebFetchPermissionRequest
        request={makeRequest({ input: { url: 'https://example.com' } })}
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
      <WebFetchPermissionRequest
        request={makeRequest({ input: { url: 'https://example.com' } })}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });
});
