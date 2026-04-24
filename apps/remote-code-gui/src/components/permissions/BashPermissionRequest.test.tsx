import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { BashPermissionRequest } from './BashPermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-bash-1',
    tool_name: 'Bash',
    tool_use_id: 'tool-1',
    title: 'Bash Command',
    description: 'Execute a bash command',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('BashPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <BashPermissionRequest
        request={makeRequest({ input: { command: 'ls -la' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByTestId('bash-permission-request')).toBeInTheDocument();
  });

  it('displays the command content', () => {
    render(
      <BashPermissionRequest
        request={makeRequest({ input: { command: 'npm test' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('npm test')).toBeInTheDocument();
  });

  it('shows dangerous command warning for rm -rf', () => {
    render(
      <BashPermissionRequest
        request={makeRequest({ input: { command: 'rm -rf /' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('危险命令')).toBeInTheDocument();
  });

  it('does not show dangerous warning for safe commands', () => {
    render(
      <BashPermissionRequest
        request={makeRequest({ input: { command: 'echo hello' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.queryByText('危险命令')).not.toBeInTheDocument();
  });

  it('shows no command content when input is empty', () => {
    render(
      <BashPermissionRequest
        request={makeRequest({ input: {} })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('无命令内容')).toBeInTheDocument();
  });

  it('calls onAllow when allow button is clicked', () => {
    const onAllow = vi.fn();
    render(
      <BashPermissionRequest
        request={makeRequest({ input: { command: 'ls' } })}
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
      <BashPermissionRequest
        request={makeRequest({ input: { command: 'ls' } })}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });

  it('supports session mode radio toggle', () => {
    render(
      <BashPermissionRequest
        request={makeRequest({ input: { command: 'ls' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    const radios = screen.getAllByRole('radio');
    const sessionRadio = radios[1];
    fireEvent.click(sessionRadio);
    expect(screen.getByText('会话允许')).toBeInTheDocument();
  });
});
