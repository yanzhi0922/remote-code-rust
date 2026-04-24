import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { SandboxViolationExpandedView } from './SandboxViolationExpandedView';

afterEach(() => {
  cleanup();
});

describe('SandboxViolationExpandedView', () => {
  it('renders nothing when no violations', () => {
    const { container } = render(<SandboxViolationExpandedView violations={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders violations', () => {
    const violations = [
      { path: '/etc/passwd', operation: 'read', denied: true },
      { path: '/tmp/test', operation: 'write', denied: false },
    ];
    render(<SandboxViolationExpandedView violations={violations} />);
    expect(screen.getByTestId('sandbox-violation-expanded')).toBeInTheDocument();
    expect(screen.getByText('/etc/passwd')).toBeInTheDocument();
    expect(screen.getByText('已拒绝')).toBeInTheDocument();
    expect(screen.getByText('警告')).toBeInTheDocument();
  });
});
