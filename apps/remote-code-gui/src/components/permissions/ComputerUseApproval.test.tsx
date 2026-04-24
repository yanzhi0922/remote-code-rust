import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { ComputerUseApproval } from './ComputerUseApproval';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-cu-1',
    tool_name: 'ComputerUse',
    tool_use_id: 'tool-cu-1',
    title: 'Computer Use',
    description: 'Computer use operation',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('ComputerUseApproval', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <ComputerUseApproval
        request={makeRequest({ input: { action: 'screenshot' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByTestId('computer-use-approval')).toBeInTheDocument();
  });

  it('displays screenshot action label', () => {
    render(
      <ComputerUseApproval
        request={makeRequest({ input: { action: 'screenshot' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('截屏')).toBeInTheDocument();
  });

  it('displays click action label', () => {
    render(
      <ComputerUseApproval
        request={makeRequest({ input: { action: 'click' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('点击')).toBeInTheDocument();
  });

  it('displays type action label', () => {
    render(
      <ComputerUseApproval
        request={makeRequest({ input: { action: 'type' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('输入')).toBeInTheDocument();
  });

  it('displays scroll action label', () => {
    render(
      <ComputerUseApproval
        request={makeRequest({ input: { action: 'scroll' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('滚动')).toBeInTheDocument();
  });

  it('shows unknown action for missing action', () => {
    render(
      <ComputerUseApproval
        request={makeRequest({ input: {} })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('未知操作')).toBeInTheDocument();
  });

  it('calls onAllow when allow button is clicked', () => {
    const onAllow = vi.fn();
    render(
      <ComputerUseApproval
        request={makeRequest({ input: { action: 'click' } })}
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
      <ComputerUseApproval
        request={makeRequest({ input: { action: 'click' } })}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });
});
