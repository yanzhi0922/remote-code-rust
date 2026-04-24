import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AgentList } from './AgentList';

const AGENTS = [
  {
    name: 'coder',
    description: '代码编写助手',
    model: 'claude-sonnet-4',
    color: '#3b82f6',
    tools: ['Bash', 'FileEdit'],
    is_builtin: false,
    disabled: false,
  },
  {
    name: 'reviewer',
    description: '代码审查专家',
    model: 'gpt-4o',
    color: '#22c55e',
    tools: ['FileRead'],
    is_builtin: true,
    disabled: false,
  },
  {
    name: 'tester',
    description: '测试生成工具',
    color: '#f97316',
    tools: [],
    is_builtin: false,
    disabled: true,
  },
];

describe('AgentList', () => {
  afterEach(cleanup);

  it('renders all agents', () => {
    render(<AgentList agents={AGENTS} onSelectAgent={vi.fn()} onCreateAgent={vi.fn()} />);
    expect(screen.getByText('coder')).toBeInTheDocument();
    expect(screen.getByText('reviewer')).toBeInTheDocument();
    expect(screen.getByText('tester')).toBeInTheDocument();
  });

  it('shows empty state when no agents', () => {
    render(<AgentList agents={[]} onSelectAgent={vi.fn()} onCreateAgent={vi.fn()} />);
    expect(screen.getByText('暂无自定义 Agent')).toBeInTheDocument();
  });

  it('calls onCreateAgent when create button is clicked', () => {
    const onCreateAgent = vi.fn();
    render(<AgentList agents={AGENTS} onSelectAgent={vi.fn()} onCreateAgent={onCreateAgent} />);
    fireEvent.click(screen.getByText('创建 Agent'));
    expect(onCreateAgent).toHaveBeenCalledTimes(1);
  });

  it('calls onSelectAgent with agent name when card is clicked', () => {
    const onSelectAgent = vi.fn();
    render(<AgentList agents={AGENTS} onSelectAgent={onSelectAgent} onCreateAgent={vi.fn()} />);
    fireEvent.click(screen.getByTestId('agent-card-coder'));
    expect(onSelectAgent).toHaveBeenCalledWith('coder');
  });

  it('filters agents by search query', () => {
    render(<AgentList agents={AGENTS} onSelectAgent={vi.fn()} onCreateAgent={vi.fn()} />);
    const searchInput = screen.getByLabelText('搜索 Agent');
    fireEvent.change(searchInput, { target: { value: '审查' } });
    expect(screen.queryByText('coder')).not.toBeInTheDocument();
    expect(screen.getByText('reviewer')).toBeInTheDocument();
  });

  it('shows empty state when search matches nothing', () => {
    render(<AgentList agents={AGENTS} onSelectAgent={vi.fn()} onCreateAgent={vi.fn()} />);
    const searchInput = screen.getByLabelText('搜索 Agent');
    fireEvent.change(searchInput, { target: { value: 'xyz' } });
    expect(screen.getByText('暂无自定义 Agent')).toBeInTheDocument();
  });
});
