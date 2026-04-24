import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AgentsList, type ResolvedAgent } from './AgentsList';

const baseAgents: ResolvedAgent[] = [
  { name: 'Code Reviewer', source: 'project', description: 'Reviews code quality', model: 'gpt-4' },
  { name: 'Security Scanner', source: 'user', description: 'Scans for vulnerabilities', memoryCount: 3 },
  { name: 'General Assistant', source: 'built-in', description: 'Default assistant' },
  { name: 'Plugin Agent', source: 'plugin', overriddenBy: 'user' },
];

describe('AgentsList', () => {
  afterEach(() => { cleanup(); });

  it('renders with data-testid', () => {
    render(<AgentsList agents={baseAgents} />);
    expect(screen.getByTestId('agents-list')).toBeInTheDocument();
  });

  it('renders all agents', () => {
    render(<AgentsList agents={baseAgents} />);
    expect(screen.getByText('Code Reviewer')).toBeInTheDocument();
    expect(screen.getByText('Security Scanner')).toBeInTheDocument();
    expect(screen.getByText('General Assistant')).toBeInTheDocument();
    expect(screen.getByText('Plugin Agent')).toBeInTheDocument();
  });

  it('renders agent descriptions', () => {
    render(<AgentsList agents={baseAgents} />);
    expect(screen.getByText('Reviews code quality')).toBeInTheDocument();
    expect(screen.getByText('Scans for vulnerabilities')).toBeInTheDocument();
  });

  it('renders model badge', () => {
    render(<AgentsList agents={baseAgents} />);
    expect(screen.getByText('gpt-4')).toBeInTheDocument();
  });

  it('renders memory count', () => {
    render(<AgentsList agents={baseAgents} />);
    expect(screen.getByText('3')).toBeInTheDocument();
  });

  it('renders override warning', () => {
    render(<AgentsList agents={baseAgents} />);
    expect(screen.getByTestId('agent-overridden-Plugin Agent')).toBeInTheDocument();
  });

  it('renders source filter tabs', () => {
    render(<AgentsList agents={baseAgents} />);
    expect(screen.getByTestId('agents-source-filter')).toBeInTheDocument();
    expect(screen.getByTestId('agents-filter-all')).toBeInTheDocument();
    expect(screen.getByTestId('agents-filter-built-in')).toBeInTheDocument();
    expect(screen.getByTestId('agents-filter-project')).toBeInTheDocument();
    expect(screen.getByTestId('agents-filter-user')).toBeInTheDocument();
    expect(screen.getByTestId('agents-filter-plugin')).toBeInTheDocument();
  });

  it('filters agents by source', () => {
    render(<AgentsList agents={baseAgents} />);
    fireEvent.click(screen.getByTestId('agents-filter-project'));
    expect(screen.getByText('Code Reviewer')).toBeInTheDocument();
    expect(screen.queryByText('Security Scanner')).not.toBeInTheDocument();
  });

  it('renders search input', () => {
    render(<AgentsList agents={baseAgents} />);
    expect(screen.getByTestId('agents-search-input')).toBeInTheDocument();
  });

  it('filters agents by search query', () => {
    render(<AgentsList agents={baseAgents} />);
    fireEvent.change(screen.getByTestId('agents-search-input'), { target: { value: 'Code' } });
    expect(screen.getByText('Code Reviewer')).toBeInTheDocument();
    expect(screen.queryByText('Security Scanner')).not.toBeInTheDocument();
  });

  it('clears search on X button click', () => {
    render(<AgentsList agents={baseAgents} />);
    const input = screen.getByTestId('agents-search-input') as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'test' } });
    expect(input.value).toBe('test');
    fireEvent.click(screen.getByLabelText('清除搜索'));
    expect(input.value).toBe('');
  });

  it('renders add button when onAdd is provided', () => {
    render(<AgentsList agents={baseAgents} onAdd={vi.fn()} />);
    expect(screen.getByTestId('agents-list-add')).toBeInTheDocument();
  });

  it('calls onAdd when add button is clicked', () => {
    const fn = vi.fn();
    render(<AgentsList agents={baseAgents} onAdd={fn} />);
    fireEvent.click(screen.getByTestId('agents-list-add'));
    expect(fn).toHaveBeenCalled();
  });

  it('renders create new button at bottom', () => {
    render(<AgentsList agents={baseAgents} onAdd={vi.fn()} />);
    expect(screen.getByTestId('agents-list-create-new-bottom')).toBeInTheDocument();
  });

  it('calls onSelect when agent is clicked', () => {
    const fn = vi.fn();
    render(<AgentsList agents={baseAgents} onSelect={fn} />);
    fireEvent.click(screen.getByTestId('agents-list-item-Code Reviewer'));
    expect(fn).toHaveBeenCalled();
  });

  it('renders selected agent highlight', () => {
    render(<AgentsList agents={baseAgents} selectedId="Code Reviewer" onSelect={vi.fn()} />);
    const item = screen.getByTestId('agents-list-item-Code Reviewer');
    expect(item.className).toContain('bg-blue-50');
  });

  it('shows empty state when no agents', () => {
    render(<AgentsList agents={[]} onAdd={vi.fn()} />);
    expect(screen.getByTestId('agents-list-empty')).toBeInTheDocument();
  });

  it('shows create new in empty state', () => {
    render(<AgentsList agents={[]} onAdd={vi.fn()} />);
    expect(screen.getByTestId('agents-list-create-new')).toBeInTheDocument();
  });

  it('renders source group headers', () => {
    render(<AgentsList agents={baseAgents} />);
    // Should have group data-testid for each source group
    expect(screen.getByTestId('agents-group-project')).toBeInTheDocument();
    expect(screen.getByTestId('agents-group-user')).toBeInTheDocument();
    expect(screen.getByTestId('agents-group-built-in')).toBeInTheDocument();
  });

  it('renders agent count in header', () => {
    render(<AgentsList agents={baseAgents} />);
    expect(screen.getByText(/Agent 列表/)).toBeInTheDocument();
  });

  it('sorts agents by name', () => {
    const unsorted: ResolvedAgent[] = [
      { name: 'Zebra', source: 'project' },
      { name: 'Alpha', source: 'project' },
    ];
    render(<AgentsList agents={unsorted} />);
    // Both are in the project group, sorted alphabetically
    expect(screen.getByTestId('agents-list-item-Alpha')).toBeInTheDocument();
    expect(screen.getByTestId('agents-list-item-Zebra')).toBeInTheDocument();
  });
});
