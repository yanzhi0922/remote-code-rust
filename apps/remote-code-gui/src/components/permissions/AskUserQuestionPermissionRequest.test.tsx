import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { AskUserQuestionPermissionRequest } from './AskUserQuestionPermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-1',
    tool_name: 'AskUser',
    tool_use_id: 'tool-1',
    title: 'Question',
    description: 'Please answer',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('AskUserQuestionPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<AskUserQuestionPermissionRequest request={makeRequest()} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('ask-user-question-permission')).toBeInTheDocument();
  });

  it('calls onAllow when answer clicked', () => {
    const onAllow = vi.fn();
    render(<AskUserQuestionPermissionRequest request={makeRequest()} onAllow={onAllow} onReject={vi.fn()} />);
    fireEvent.click(screen.getByText('回答'));
    expect(onAllow).toHaveBeenCalledTimes(1);
  });

  it('calls onReject when skip clicked', () => {
    const onReject = vi.fn();
    render(<AskUserQuestionPermissionRequest request={makeRequest()} onAllow={vi.fn()} onReject={onReject} />);
    fireEvent.click(screen.getByText('跳过'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });
});
