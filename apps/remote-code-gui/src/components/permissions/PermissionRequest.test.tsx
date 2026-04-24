import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequestInfo } from '../../lib/types';
import { PermissionRequest } from './PermissionRequest';

const base: PermissionRequestInfo = {
  request_id: 'r1',
  tool_name: 'Tool',
  tool_use_id: 't1',
  title: 'Test',
  description: 'Desc',
  input: {},
  blocked_path: null,
  permission_suggestions: [],
};

describe('PermissionRequest', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<PermissionRequest request={base} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('permission-request')).toBeInTheDocument();
  });

  it('calls onAllow', () => {
    const fn = vi.fn();
    render(<PermissionRequest request={base} onAllow={fn} onReject={vi.fn()} />);
    fireEvent.click(screen.getByTestId('permission-allow'));
    expect(fn).toHaveBeenCalled();
  });

  it('calls onReject', () => {
    const fn = vi.fn();
    render(<PermissionRequest request={base} onAllow={vi.fn()} onReject={fn} />);
    fireEvent.click(screen.getByTestId('permission-reject'));
    expect(fn).toHaveBeenCalled();
  });

  // ── Bash 权限 ──

  it('renders bash permission detail for Bash tool', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'Bash',
      input: { command: 'ls -la', cwd: '/home/user' },
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('bash-permission-detail')).toBeInTheDocument();
    expect(screen.getByText('ls -la', { exact: false })).toBeInTheDocument();
  });

  it('shows dangerous command warning', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'Bash',
      input: { command: 'rm -rf /tmp/test' },
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByText('检测到潜在危险命令')).toBeInTheDocument();
  });

  it('does not show warning for safe command', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'Bash',
      input: { command: 'ls -la' },
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.queryByText('检测到潜在危险命令')).not.toBeInTheDocument();
  });

  it('shows working directory for bash command', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'Bash',
      input: { command: 'ls', cwd: '/home/user/project' },
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByText('/home/user/project')).toBeInTheDocument();
  });

  // ── FileEdit 权限 ──

  it('renders file edit permission detail', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'FileEdit',
      input: {
        file_path: '/src/app.tsx',
        old_string: 'old code',
        new_string: 'new code',
      },
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('file-edit-permission-detail')).toBeInTheDocument();
    expect(screen.getByText('/src/app.tsx')).toBeInTheDocument();
  });

  it('shows replace all warning', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'FileEdit',
      input: {
        file_path: '/src/app.tsx',
        old_string: 'old',
        new_string: 'new',
        replace_all: true,
      },
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByText('⚠ 将替换所有匹配项')).toBeInTheDocument();
  });

  // ── FileWrite 权限 ──

  it('renders file write permission detail', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'FileWrite',
      input: {
        file_path: '/src/new-file.ts',
        content: 'export const hello = "world";',
      },
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('file-write-permission-detail')).toBeInTheDocument();
    expect(screen.getByText('/src/new-file.ts')).toBeInTheDocument();
  });

  // ── MCP 权限 ──

  it('renders MCP permission detail', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'mcp_tool_use',
      input: {
        server_name: 'my-server',
        tool_name: 'search',
        arguments: { query: 'test' },
      },
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('mcp-permission-detail')).toBeInTheDocument();
    expect(screen.getByText('my-server')).toBeInTheDocument();
    expect(screen.getByText('search')).toBeInTheDocument();
  });

  // ── WebFetch 权限 ──

  it('renders web fetch permission detail', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'WebFetch',
      input: { url: 'https://example.com/api' },
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('webfetch-permission-detail')).toBeInTheDocument();
    expect(screen.getByText('https://example.com/api')).toBeInTheDocument();
  });

  // ── 通用降级 ──

  it('renders fallback for unknown tool', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'UnknownTool',
      input: { foo: 'bar' },
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('fallback-permission-detail')).toBeInTheDocument();
  });

  // ── Worker badge ──

  it('shows worker badge when provided', () => {
    render(
      <PermissionRequest
        request={base}
        onAllow={vi.fn()}
        onReject={vi.fn()}
        workerBadge={{ name: 'worker-1', color: '#ff0000' }}
      />,
    );
    expect(screen.getByText('@worker-1')).toBeInTheDocument();
  });

  // ── 权限建议 ──

  it('shows permission suggestions', () => {
    const req: PermissionRequestInfo = {
      ...base,
      permission_suggestions: ['Allow Bash for this project', 'Add to allowlist'],
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('permission-suggestions')).toBeInTheDocument();
    expect(screen.getByText('Allow Bash for this project')).toBeInTheDocument();
  });

  it('does not show suggestions section when empty', () => {
    render(<PermissionRequest request={base} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.queryByTestId('permission-suggestions')).not.toBeInTheDocument();
  });

  // ── 阻塞路径 ──

  it('shows blocked path when present', () => {
    const req: PermissionRequestInfo = {
      ...base,
      blocked_path: '/etc/passwd',
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByText('/etc/passwd')).toBeInTheDocument();
  });

  // ── 反馈输入 ──

  it('shows feedback input on toggle', () => {
    render(<PermissionRequest request={base} onAllow={vi.fn()} onReject={vi.fn()} />);
    fireEvent.click(screen.getByTestId('permission-show-feedback'));
    expect(screen.getByTestId('permission-feedback')).toBeInTheDocument();
  });

  it('calls onReject with feedback text', () => {
    const fn = vi.fn();
    render(<PermissionRequest request={base} onAllow={vi.fn()} onReject={fn} />);
    fireEvent.click(screen.getByTestId('permission-show-feedback'));
    fireEvent.change(screen.getByTestId('permission-feedback-input'), {
      target: { value: 'too dangerous' },
    });
    fireEvent.click(screen.getByTestId('permission-reject'));
    expect(fn).toHaveBeenCalledWith('too dangerous');
  });

  // ── 边界情况 ──

  it('handles empty input gracefully', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'Bash',
      input: {},
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('bash-permission-detail')).toBeInTheDocument();
  });

  it('handles null input gracefully', () => {
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'UnknownTool',
      input: null,
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('fallback-permission-detail')).toBeInTheDocument();
  });

  it('handles very long command', () => {
    const longCmd = 'echo ' + 'a'.repeat(1000);
    const req: PermissionRequestInfo = {
      ...base,
      tool_name: 'Bash',
      input: { command: longCmd },
    };
    render(<PermissionRequest request={req} onAllow={vi.fn()} onReject={vi.fn()} />);
    expect(screen.getByTestId('bash-permission-detail')).toBeInTheDocument();
  });
});
