import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PermissionRuleDescription } from './PermissionRuleDescription';

describe('PermissionRuleDescription', () => {
  afterEach(cleanup);

  it('shows "Any use" when rule_content is empty', () => {
    render(
      <PermissionRuleDescription
        ruleValue={{ tool_name: 'Bash', rule_content: '', behavior: 'allow' }}
      />,
    );
    expect(screen.getByText(/Any use of the Bash tool/)).toBeInTheDocument();
  });

  it('shows semantic rule for prompt: prefix', () => {
    render(
      <PermissionRuleDescription
        ruleValue={{
          tool_name: 'Bash',
          rule_content: 'prompt: run tests safely',
          behavior: 'allow',
        }}
      />,
    );
    expect(screen.getByText(/Semantic rule: "run tests safely"/)).toBeInTheDocument();
  });

  it('shows glob pattern for :* suffix', () => {
    render(
      <PermissionRuleDescription
        ruleValue={{ tool_name: 'Bash', rule_content: 'git:*', behavior: 'allow' }}
      />,
    );
    expect(screen.getByText(/Any Bash command starting with "git"/)).toBeInTheDocument();
  });

  it('shows pattern matching for glob characters', () => {
    render(
      <PermissionRuleDescription
        ruleValue={{
          tool_name: 'Edit',
          rule_content: 'src/**/*.ts',
          behavior: 'deny',
        }}
      />,
    );
    expect(screen.getByText(/Edit matching pattern "src\/\*\*\/\*\.ts"/)).toBeInTheDocument();
  });

  it('shows exact match for plain rule content', () => {
    render(
      <PermissionRuleDescription
        ruleValue={{ tool_name: 'Bash', rule_content: 'npm test', behavior: 'ask' }}
      />,
    );
    expect(screen.getByText(/The Bash command "npm test"/)).toBeInTheDocument();
  });

  it('shows allow badge with green color', () => {
    render(
      <PermissionRuleDescription
        ruleValue={{ tool_name: 'Bash', rule_content: '', behavior: 'allow' }}
      />,
    );
    const badge = screen.getByText('Allow');
    expect(badge.className).toContain('bg-emerald-50');
  });

  it('shows deny badge with red color', () => {
    render(
      <PermissionRuleDescription
        ruleValue={{ tool_name: 'Bash', rule_content: '', behavior: 'deny' }}
      />,
    );
    const badge = screen.getByText('Deny');
    expect(badge.className).toContain('bg-red-50');
  });

  it('shows ask badge with amber color', () => {
    render(
      <PermissionRuleDescription
        ruleValue={{ tool_name: 'Bash', rule_content: '', behavior: 'ask' }}
      />,
    );
    const badge = screen.getByText('Ask');
    expect(badge.className).toContain('bg-amber-50');
  });
});
