import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PermissionRuleExplanation } from './PermissionRuleExplanation';

describe('PermissionRuleExplanation', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(
      <PermissionRuleExplanation rule={{ tool: 'Bash', behavior: 'allow' }} />,
    );
    expect(screen.getByTestId('permission-rule-explanation')).toBeInTheDocument();
  });

  it('displays allow rule with pattern', () => {
    render(
      <PermissionRuleExplanation
        rule={{ tool: 'Bash', behavior: 'allow', pattern: 'npm test' }}
      />,
    );
    expect(screen.getByText(/Allow Bash matching 'npm test'/)).toBeInTheDocument();
  });

  it('displays deny rule without pattern', () => {
    render(
      <PermissionRuleExplanation rule={{ tool: 'FileEdit', behavior: 'deny' }} />,
    );
    expect(screen.getByText('Deny FileEdit')).toBeInTheDocument();
  });

  it('shows green styling for allow behavior', () => {
    render(
      <PermissionRuleExplanation rule={{ tool: 'Bash', behavior: 'allow' }} />,
    );
    const container = screen.getByTestId('permission-rule-explanation');
    expect(container.className).toContain('bg-emerald-50');
  });

  it('shows red styling for deny behavior', () => {
    render(
      <PermissionRuleExplanation rule={{ tool: 'Bash', behavior: 'deny' }} />,
    );
    const container = screen.getByTestId('permission-rule-explanation');
    expect(container.className).toContain('bg-red-50');
  });

  it('handles case-insensitive behavior', () => {
    render(
      <PermissionRuleExplanation rule={{ tool: 'Write', behavior: 'ALLOW' }} />,
    );
    expect(screen.getByText(/Allow Write/)).toBeInTheDocument();
    const container = screen.getByTestId('permission-rule-explanation');
    expect(container.className).toContain('bg-emerald-50');
  });

  it('displays deny rule with pattern', () => {
    render(
      <PermissionRuleExplanation
        rule={{ tool: 'FileEdit', behavior: 'deny', pattern: '*.secret' }}
      />,
    );
    expect(screen.getByText(/Deny FileEdit matching '\*\.secret'/)).toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(
      <PermissionRuleExplanation
        rule={{ tool: 'Bash', behavior: 'allow' }}
        className="mt-4"
      />,
    );
    const container = screen.getByTestId('permission-rule-explanation');
    expect(container.className).toContain('mt-4');
  });
});
