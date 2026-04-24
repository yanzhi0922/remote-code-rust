import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PermissionExplanation } from './PermissionExplanation';

describe('PermissionExplanation', () => {
  afterEach(cleanup);

  it('shows toggle button when not visible', () => {
    const onToggle = vi.fn();
    render(
      <PermissionExplanation
        toolName="Bash"
        toolInput={{ command: 'ls' }}
        visible={false}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByText('查看风险说明 (Ctrl+E)')).toBeInTheDocument();
  });

  it('calls onToggle when toggle button is clicked', () => {
    const onToggle = vi.fn();
    render(
      <PermissionExplanation
        toolName="Bash"
        toolInput={{ command: 'ls' }}
        visible={false}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByText('查看风险说明 (Ctrl+E)'));
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it('shows loading shimmer when visible and loading', () => {
    render(
      <PermissionExplanation
        toolName="Bash"
        toolInput={{ command: 'ls' }}
        visible={true}
        onToggle={vi.fn()}
      />,
    );
    expect(screen.getByTestId('explanation-loading')).toBeInTheDocument();
  });

  it('shows risk result after loading completes', async () => {
    render(
      <PermissionExplanation
        toolName="Bash"
        toolInput={{ command: 'ls' }}
        visible={true}
        onToggle={vi.fn()}
      />,
    );

    await waitFor(
      () => {
        expect(screen.getByTestId('explanation-result')).toBeInTheDocument();
      },
      { timeout: 2000 },
    );
    expect(screen.getByText('Low risk')).toBeInTheDocument();
  });

  it('shows HIGH risk for dangerous commands', async () => {
    render(
      <PermissionExplanation
        toolName="Bash"
        toolInput={{ command: 'rm -rf /' }}
        visible={true}
        onToggle={vi.fn()}
      />,
    );

    await waitFor(
      () => {
        expect(screen.getByText('High risk')).toBeInTheDocument();
      },
      { timeout: 2000 },
    );
  });

  it('shows MEDIUM risk for network commands', async () => {
    render(
      <PermissionExplanation
        toolName="Bash"
        toolInput={{ command: 'curl https://example.com' }}
        visible={true}
        onToggle={vi.fn()}
      />,
    );

    await waitFor(
      () => {
        expect(screen.getByText('Medium risk')).toBeInTheDocument();
      },
      { timeout: 2000 },
    );
  });

  it('shows collapse button when visible', () => {
    render(
      <PermissionExplanation
        toolName="Bash"
        toolInput={{ command: 'ls' }}
        visible={true}
        onToggle={vi.fn()}
      />,
    );
    expect(screen.getByText('收起风险说明')).toBeInTheDocument();
  });
});
