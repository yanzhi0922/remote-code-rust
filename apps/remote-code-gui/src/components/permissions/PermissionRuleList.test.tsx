import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PermissionRuleList, type PermissionRule } from './PermissionRuleList';

const baseRules: PermissionRule[] = [
  { id: 'r1', tool_name: 'Bash', rule_content: 'ls *', behavior: 'allow', source: 'project' },
  { id: 'r2', tool_name: 'Edit', rule_content: 'src/**', behavior: 'deny', source: 'user' },
  { id: 'r3', tool_name: 'Read', rule_content: '', behavior: 'ask', source: 'workspace' },
  { id: 'r4', tool_name: 'Bash', rule_content: 'rm *', behavior: 'deny', source: 'managed', isManaged: true },
  { id: 'r5', tool_name: 'Write', rule_content: '*.ts', behavior: 'allow', source: 'project', overriddenBy: 'user' },
];

describe('PermissionRuleList', () => {
  afterEach(() => { cleanup(); });

  it('renders with data-testid', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    expect(screen.getByTestId('rule-list')).toBeInTheDocument();
  });

  it('renders all tabs', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    expect(screen.getByTestId('tab-allow')).toBeInTheDocument();
    expect(screen.getByTestId('tab-deny')).toBeInTheDocument();
    expect(screen.getByTestId('tab-ask')).toBeInTheDocument();
    expect(screen.getByTestId('tab-recent')).toBeInTheDocument();
    expect(screen.getByTestId('tab-workspace')).toBeInTheDocument();
  });

  it('shows rule counts on tabs', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    // The allow tab should show count of allow rules
    const allowTab = screen.getByTestId('tab-allow');
    expect(allowTab.textContent).toContain('2'); // r1 and r5 are allow
  });

  it('filters rules by tab', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    // Default tab is 'allow', should show allow rules
    expect(screen.getByTestId('rule-item-r1')).toBeInTheDocument();
    expect(screen.getByTestId('rule-item-r5')).toBeInTheDocument();
    // Deny rules should not be visible
    expect(screen.queryByTestId('rule-item-r2')).not.toBeInTheDocument();
  });

  it('switches tabs on click', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    fireEvent.click(screen.getByTestId('tab-deny'));
    expect(screen.getByTestId('rule-item-r2')).toBeInTheDocument();
    expect(screen.getByTestId('rule-item-r4')).toBeInTheDocument();
  });

  it('renders search input', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    expect(screen.getByTestId('rule-search-input')).toBeInTheDocument();
  });

  it('filters rules by search query', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    fireEvent.change(screen.getByTestId('rule-search-input'), { target: { value: 'Bash' } });
    // Should show Bash rules in allow tab
    expect(screen.getByTestId('rule-item-r1')).toBeInTheDocument();
    expect(screen.queryByTestId('rule-item-r5')).not.toBeInTheDocument();
  });

  it('shows empty state for empty tab', () => {
    render(<PermissionRuleList rules={[]} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    expect(screen.getByTestId('rule-empty-state')).toBeInTheDocument();
  });

  it('shows managed badge for managed rules', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    fireEvent.click(screen.getByTestId('tab-deny'));
    expect(screen.getByTestId('rule-managed-r4')).toBeInTheDocument();
  });

  it('shows override warning for overridden rules', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    expect(screen.getByTestId('rule-overridden-r5')).toBeInTheDocument();
  });

  it('expands rule detail on toggle click', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    expect(screen.queryByTestId('rule-detail-r1')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId('rule-toggle-r1'));
    expect(screen.getByTestId('rule-detail-r1')).toBeInTheDocument();
  });

  it('collapses rule detail on second toggle click', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    fireEvent.click(screen.getByTestId('rule-toggle-r1'));
    expect(screen.getByTestId('rule-detail-r1')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('rule-toggle-r1'));
    expect(screen.queryByTestId('rule-detail-r1')).not.toBeInTheDocument();
  });

  it('shows delete confirmation on delete click', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    fireEvent.click(screen.getByTestId('rule-delete-r1'));
    expect(screen.getByTestId('rule-delete-confirm-r1')).toBeInTheDocument();
  });

  it('calls onDelete on confirm delete', () => {
    const fn = vi.fn();
    render(<PermissionRuleList rules={baseRules} onDelete={fn} onAddRule={vi.fn()} />);
    fireEvent.click(screen.getByTestId('rule-delete-r1'));
    fireEvent.click(screen.getByTestId('rule-delete-confirm-yes-r1'));
    expect(fn).toHaveBeenCalledWith('r1');
  });

  it('cancels delete on cancel click', () => {
    const fn = vi.fn();
    render(<PermissionRuleList rules={baseRules} onDelete={fn} onAddRule={vi.fn()} />);
    fireEvent.click(screen.getByTestId('rule-delete-r1'));
    fireEvent.click(screen.getByTestId('rule-delete-confirm-no-r1'));
    expect(fn).not.toHaveBeenCalled();
    expect(screen.queryByTestId('rule-delete-confirm-r1')).not.toBeInTheDocument();
  });

  it('calls onAddRule on add button click', () => {
    const fn = vi.fn();
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={fn} />);
    fireEvent.click(screen.getByTestId('rule-add-button'));
    expect(fn).toHaveBeenCalled();
  });

  it('renders sort button', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    expect(screen.getByTestId('rule-sort-button')).toBeInTheDocument();
  });

  it('cycles sort mode on sort button click', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    const btn = screen.getByTestId('rule-sort-button');
    expect(btn.textContent).toContain('默认排序');
    fireEvent.click(btn);
    expect(btn.textContent).toContain('按来源');
  });

  it('does not show delete button for managed rules', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    fireEvent.click(screen.getByTestId('tab-deny'));
    expect(screen.queryByTestId('rule-delete-r4')).not.toBeInTheDocument();
  });

  it('renders footer summary', () => {
    render(<PermissionRuleList rules={baseRules} onDelete={vi.fn()} onAddRule={vi.fn()} />);
    const container = screen.getByTestId('rule-items-container');
    expect(container.parentElement?.textContent).toContain('共 5 条规则');
  });
});
