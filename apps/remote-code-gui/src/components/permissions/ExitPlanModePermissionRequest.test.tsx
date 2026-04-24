import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { ExitPlanModePermissionRequest } from './ExitPlanModePermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-exit-plan-1',
    tool_name: 'ExitPlanMode',
    tool_use_id: 'tool-exit-plan-1',
    title: '退出计划模式',
    description: 'Request to exit plan mode',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('ExitPlanModePermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <ExitPlanModePermissionRequest
        request={makeRequest()}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByTestId('exit-plan-mode-request')).toBeInTheDocument();
  });

  it('displays the title', () => {
    render(
      <ExitPlanModePermissionRequest
        request={makeRequest({ title: 'Exit Plan' })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('Exit Plan')).toBeInTheDocument();
  });

  it('shows default title when not provided', () => {
    render(
      <ExitPlanModePermissionRequest
        request={makeRequest({ title: '' })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('退出计划模式')).toBeInTheDocument();
  });

  it('displays plan summary when provided', () => {
    render(
      <ExitPlanModePermissionRequest
        request={makeRequest({ input: { plan: '1. Create files\n2. Run tests' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('计划摘要:')).toBeInTheDocument();
    expect(screen.getByText(/Create files/)).toBeInTheDocument();
  });

  it('shows exit message when no plan summary', () => {
    render(
      <ExitPlanModePermissionRequest
        request={makeRequest({ input: {} })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText(/退出计划模式并开始执行任务/)).toBeInTheDocument();
  });

  it('calls onAllow when allow button is clicked', () => {
    const onAllow = vi.fn();
    render(
      <ExitPlanModePermissionRequest
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
      <ExitPlanModePermissionRequest
        request={makeRequest()}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });

  it('has emerald border styling', () => {
    render(
      <ExitPlanModePermissionRequest
        request={makeRequest()}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    const container = screen.getByTestId('exit-plan-mode-request');
    expect(container.className).toContain('border-emerald-300');
  });
});
