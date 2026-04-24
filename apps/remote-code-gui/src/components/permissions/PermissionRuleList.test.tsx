import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PermissionRuleList, type PermissionRule } from './PermissionRuleList';

const sampleRules: PermissionRule[] = [
  {
    id: 'rule-1',
    tool_name: 'Bash',
    rule_content: 'npm test',
    behavior: 'allow',
    source: 'session',
  },
  {
    id: 'rule-2',
    tool_name: 'Edit',
    rule_content: 'src/**/*.ts',
    behavior: 'deny',
    source: 'workspace',
  },
  {
    id: 'rule-3',
    tool_name: 'Bash',
    rule_content: 'git:*',
    behavior: 'ask',
    source: 'project',
  },
  {
    id: 'rule-4',
    tool_name: 'Bash',
    rule_content: 'rm -rf /',
    behavior: 'deny',
    source: 'session',
  },
];

describe('PermissionRuleList', () => {
  afterEach(cleanup);

  it('renders all tabs', () => {
    render(
      <PermissionRuleList rules={sampleRules} onDelete={vi.fn()} onAddRule={vi.fn()} />,
    );
    expect(screen.getByText('最近拒绝')).toBeInTheDocument();
    // Tab buttons and rule badges may share text, use getAllByText
    expect(screen.getAllByText('Allow').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Ask').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Deny').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('Workspace')).toBeInTheDocument();
  });

  it('shows allow rules by default', () => {
    render(
      <PermissionRuleList rules={sampleRules} onDelete={vi.fn()} onAddRule={vi.fn()} />,
    );
    expect(screen.getByText(/The Bash command "npm test"/)).toBeInTheDocument();
  });

  it('filters by tab - deny', () => {
    render(
      <PermissionRuleList rules={sampleRules} onDelete={vi.fn()} onAddRule={vi.fn()} />,
    );
    fireEvent.click(screen.getByText('Deny'));
    expect(screen.getByText(/Edit matching pattern/)).toBeInTheDocument();
  });

  it('filters by search query', () => {
    render(
      <PermissionRuleList rules={sampleRules} onDelete={vi.fn()} onAddRule={vi.fn()} />,
    );
    const searchInput = screen.getByPlaceholderText('搜索规则...');
    fireEvent.change(searchInput, { target: { value: 'git' } });
    // After filtering, only git:* rule should show (on ask tab)
    // But we're on allow tab, so nothing should show
    expect(screen.getByText('没有找到匹配的规则')).toBeInTheDocument();
  });

  it('calls onDelete when delete button is clicked', () => {
    const onDelete = vi.fn();
    render(
      <PermissionRuleList rules={sampleRules} onDelete={onDelete} onAddRule={vi.fn()} />,
    );
    fireEvent.click(screen.getByLabelText('删除规则 rule-1'));
    expect(onDelete).toHaveBeenCalledWith('rule-1');
  });

  it('calls onAddRule when add button is clicked', () => {
    const onAddRule = vi.fn();
    render(
      <PermissionRuleList rules={sampleRules} onDelete={vi.fn()} onAddRule={onAddRule} />,
    );
    fireEvent.click(screen.getByText('添加'));
    expect(onAddRule).toHaveBeenCalledTimes(1);
  });

  it('shows empty state when no rules match', () => {
    render(
      <PermissionRuleList rules={[]} onDelete={vi.fn()} onAddRule={vi.fn()} />,
    );
    expect(screen.getByText('暂无规则')).toBeInTheDocument();
  });

  it('shows source label for each rule', () => {
    render(
      <PermissionRuleList rules={sampleRules} onDelete={vi.fn()} onAddRule={vi.fn()} />,
    );
    expect(screen.getByText('来源: session')).toBeInTheDocument();
  });

  it('clears search when X button is clicked', () => {
    render(
      <PermissionRuleList rules={sampleRules} onDelete={vi.fn()} onAddRule={vi.fn()} />,
    );
    const searchInput = screen.getByPlaceholderText('搜索规则...') as HTMLInputElement;
    fireEvent.change(searchInput, { target: { value: 'test' } });
    expect(searchInput.value).toBe('test');
    fireEvent.click(screen.getByLabelText('清除搜索'));
    expect(searchInput.value).toBe('');
  });

  it('filters workspace tab by source', () => {
    render(
      <PermissionRuleList rules={sampleRules} onDelete={vi.fn()} onAddRule={vi.fn()} />,
    );
    fireEvent.click(screen.getByText('Workspace'));
    expect(screen.getByText(/Edit matching pattern/)).toBeInTheDocument();
  });
});
