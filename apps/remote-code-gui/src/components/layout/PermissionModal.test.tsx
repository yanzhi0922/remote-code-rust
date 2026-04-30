import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PermissionModal } from './PermissionModal';
import { resetAppStore } from '../../test/appStoreTestUtils';

describe('PermissionModal', () => {
  beforeEach(() => {
    resetAppStore();
  });

  afterEach(() => {
    cleanup();
    resetAppStore();
    vi.clearAllMocks();
  });

  it('renders the pending permission request and resolves user decisions', async () => {
    const resolvePermission = vi.fn().mockResolvedValue(undefined);

    resetAppStore({
      pendingPermission: {
        request_id: 'perm-1',
        tool_name: 'shell_command',
        tool_use_id: 'tool-1',
        title: 'Run shell command',
        description: '需要执行命令来继续修复。',
        input: { command: 'git status --short' },
        blocked_path: 'C:\\repo',
        permission_suggestions: [
          {
            action: 'allow',
            toolPattern: 'shell_command',
            pathPattern: 'C:\\repo',
          },
        ],
      },
      resolvePermission,
    });

    const { rerender } = render(<PermissionModal />);

    expect(screen.getByText('权限确认')).toBeInTheDocument();
    expect(screen.getByText('shell_command')).toBeInTheDocument();
    expect(screen.getByText('需要执行命令来继续修复。')).toBeInTheDocument();
    expect(screen.getByText('C:\\repo')).toBeInTheDocument();
    expect(screen.getByText('权限建议')).toBeInTheDocument();
    expect(screen.getByText(/"action": "allow"/)).toBeInTheDocument();
    expect(screen.getByText(/"toolPattern": "shell_command"/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '允许执行' }));
    await waitFor(() => {
      expect(resolvePermission).toHaveBeenCalledWith({ allowed: true });
    });

    resolvePermission.mockClear();
    fireEvent.click(screen.getByRole('button', { name: '拒绝' }));
    await waitFor(() => {
      expect(resolvePermission).toHaveBeenCalledWith({ allowed: false });
    });

    resetAppStore({ pendingPermission: null });
    rerender(<PermissionModal />);
    expect(screen.queryByText('权限确认')).not.toBeInTheDocument();
  });

  it('builds structured exit plan approvals with prompt-rule updates and feedback', async () => {
    const resolvePermission = vi.fn().mockResolvedValue(undefined);

    resetAppStore({
      pendingPermission: {
        request_id: 'perm-plan',
        tool_name: 'exit_plan_mode',
        tool_use_id: 'tool-plan',
        title: 'Allow ExitPlanMode',
        description: 'Claude 已经写好计划，等待批准后开始执行。',
        input: {
          plan: '# Plan\n- run tests\n- ship it\n',
          planFilePath: 'C:\\repo\\.remote-code\\plans\\ship.md',
          allowedPrompts: [{ tool: 'Bash', prompt: 'run tests' }],
        },
        blocked_path: null,
        permission_suggestions: [],
      },
      resolvePermission,
    });

    render(<PermissionModal />);

    expect(screen.getByText('请求的语义权限')).toBeInTheDocument();
    expect(screen.getByText('Bash(prompt: run tests)')).toBeInTheDocument();
    expect(screen.getByText('C:\\repo\\.remote-code\\plans\\ship.md')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('审批反馈'), {
      target: { value: 'Approved, but keep the verification step.' },
    });
    fireEvent.click(screen.getByRole('button', { name: '允许执行' }));

    await waitFor(() => {
      expect(resolvePermission).toHaveBeenCalledWith({
        allowed: true,
        feedback: 'Approved, but keep the verification step.',
        permission_updates: [
          {
            type: 'addRules',
            destination: 'session',
            behavior: 'allow',
            rules: [{ tool_name: 'Bash', rule_content: 'prompt: run tests' }],
          },
        ],
      });
    });
  });

  it('resolves Codex request_user_input with an official response payload', async () => {
    const resolvePermission = vi.fn().mockResolvedValue(undefined);

    resetAppStore({
      pendingPermission: {
        request_id: 'codex-input',
        tool_name: 'tool_user_input',
        tool_use_id: '',
        title: 'Codex 请求权限',
        description: '工具 tool_user_input 需要授权才能执行。',
        input: {
          questions: [
            {
              id: 'choice',
              header: 'Choice',
              question: 'Pick one',
              options: [{ label: 'A', description: 'Option A' }],
            },
          ],
        },
        blocked_path: null,
        permission_suggestions: [],
      },
      resolvePermission,
    });

    render(<PermissionModal />);

    expect(screen.getByText('Codex 用户输入请求')).toBeInTheDocument();
    expect(screen.getByText('Pick one')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '允许执行' }));

    await waitFor(() => {
      expect(resolvePermission).toHaveBeenCalledWith({
        allowed: true,
        codex_response: { answers: { choice: { answers: ['A'] } } },
      });
    });
  });

  it('resolves Codex MCP elicitation decline with typed response payload', async () => {
    const resolvePermission = vi.fn().mockResolvedValue(undefined);

    resetAppStore({
      pendingPermission: {
        request_id: 'codex-mcp',
        tool_name: 'mcp_elicitation',
        tool_use_id: '',
        title: 'Codex 请求权限',
        description: 'MCP elicitation',
        input: { serverName: 'memory', message: 'Need input' },
        blocked_path: null,
        permission_suggestions: [],
      },
      resolvePermission,
    });

    render(<PermissionModal />);

    expect(screen.getByText('Codex MCP elicitation')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '拒绝' }));

    await waitFor(() => {
      expect(resolvePermission).toHaveBeenCalledWith({
        allowed: false,
        codex_response: { action: 'decline', content: null, _meta: null },
      });
    });
  });
});
