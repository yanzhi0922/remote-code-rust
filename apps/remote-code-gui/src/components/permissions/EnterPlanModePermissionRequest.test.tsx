import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { EnterPlanModePermissionRequest } from './EnterPlanModePermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-enter-plan-1',
    tool_name: 'EnterPlanMode',
    tool_use_id: 'tool-plan-1',
    title: '进入计划模式',
    description: 'Request to enter plan mode',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('EnterPlanModePermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <EnterPlanModePermissionRequest
        request={makeRequest()}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByTestId('enter-plan-mode-request')).toBeInTheDocument();
  });

  it('displays the title', () => {
    render(
      <EnterPlanModePermissionRequest
        request={makeRequest({ title: 'Plan Mode Request' })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('Plan Mode Request')).toBeInTheDocument();
  });

  it('shows default title when not provided', () => {
    render(
      <EnterPlanModePermissionRequest
        request={makeRequest({ title: '' })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('进入计划模式')).toBeInTheDocument();
  });

  it('displays plan mode description', () => {
    render(
      <EnterPlanModePermissionRequest
        request={makeRequest()}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText(/请求进入计划模式/)).toBeInTheDocument();
  });

  it('calls onAllow when allow button is clicked', () => {
    const onAllow = vi.fn();
    render(
      <EnterPlanModePermissionRequest
        request={makeRequest()}
        onAllow={onAllow}
        onReject={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByText('允许'));
    expect(onAllow).toHaveBeenCalledTimes(1);
  });

  it('calls onReject when reject button is clicked', () => {
    const onReject = vi.fn();
    render(
      <EnterPlanModePermissionRequest
        request={makeRequest()}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });

  it('has blue border styling', () => {
    render(
      <EnterPlanModePermissionRequest
        request={makeRequest()}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    const container = screen.getByTestId('enter-plan-mode-request');
    expect(container.className).toContain('border-blue-300');
  });
});
