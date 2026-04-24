import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { PowerShellPermissionRequest } from './PowerShellPermissionRequest';

function makeRequest(overrides: Partial<PermissionRequestInfo> = {}): PermissionRequestInfo {
  return {
    request_id: 'req-ps-1',
    tool_name: 'PowerShell',
    tool_use_id: 'tool-ps-1',
    title: 'PowerShell Command',
    description: 'Execute a PowerShell command',
    input: {},
    blocked_path: null,
    permission_suggestions: [],
    ...overrides,
  };
}

describe('PowerShellPermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <PowerShellPermissionRequest
        request={makeRequest({ input: { command: 'Get-Process' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByTestId('powershell-permission-request')).toBeInTheDocument();
  });

  it('displays the command content', () => {
    render(
      <PowerShellPermissionRequest
        request={makeRequest({ input: { command: 'Get-ChildItem' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('Get-ChildItem')).toBeInTheDocument();
  });

  it('detects EncodedCommand as dangerous', () => {
    render(
      <PowerShellPermissionRequest
        request={makeRequest({ input: { command: 'powershell -EncodedCommand abc123' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('危险 PowerShell 命令')).toBeInTheDocument();
    expect(screen.getByText(/编码命令/)).toBeInTheDocument();
  });

  it('detects Invoke-Expression as dangerous', () => {
    render(
      <PowerShellPermissionRequest
        request={makeRequest({ input: { command: 'Invoke-Expression "ls"' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.getByText('危险 PowerShell 命令')).toBeInTheDocument();
    expect(screen.getByText(/使用 Invoke-Expression/)).toBeInTheDocument();
  });

  it('does not show warning for safe commands', () => {
    render(
      <PowerShellPermissionRequest
        request={makeRequest({ input: { command: 'Get-Process' } })}
        onAllow={vi.fn()}
        onReject={vi.fn()}
      />,
    );
    expect(screen.queryByText('危险 PowerShell 命令')).not.toBeInTheDocument();
  });

  it('shows no command content when input is empty', () => {
    render(
      <PowerShellPermissionRequest
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
      <PowerShellPermissionRequest
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
      <PowerShellPermissionRequest
        request={makeRequest({ input: { command: 'ls' } })}
        onAllow={vi.fn()}
        onReject={onReject}
      />,
    );
    fireEvent.click(screen.getByText('拒绝'));
    expect(onReject).toHaveBeenCalledTimes(1);
  });
});
