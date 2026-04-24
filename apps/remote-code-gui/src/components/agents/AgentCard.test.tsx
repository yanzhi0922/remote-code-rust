import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AgentCard } from './AgentCard';

const BASE_AGENT = {
  name: 'coder',
  description: '代码编写助手',
  model: 'claude-sonnet-4',
  color: '#3b82f6',
  tools: ['Bash', 'FileEdit', 'FileRead'],
  is_builtin: false,
  disabled: false,
};

describe('AgentCard', () => {
  afterEach(cleanup);

  it('renders agent name and description', () => {
    render(<AgentCard agent={BASE_AGENT} onClick={vi.fn()} />);
    expect(screen.getByText('coder')).toBeInTheDocument();
    expect(screen.getByText('代码编写助手')).toBeInTheDocument();
  });

  it('shows color dot with correct color', () => {
    render(<AgentCard agent={BASE_AGENT} onClick={vi.fn()} />);
    const dot = screen.getByTestId('agent-card-coder').querySelector('.rounded-full.h-3');
    expect(dot).toBeInTheDocument();
    expect(dot).toHaveStyle({ backgroundColor: '#3b82f6' });
  });

  it('shows model badge when model is set', () => {
    render(<AgentCard agent={BASE_AGENT} onClick={vi.fn()} />);
    expect(screen.getByText('claude-sonnet-4')).toBeInTheDocument();
  });

  it('shows tool count badge', () => {
    render(<AgentCard agent={BASE_AGENT} onClick={vi.fn()} />);
    expect(screen.getByText('3 工具')).toBeInTheDocument();
  });

  it('shows "内置" label for builtin agents', () => {
    render(<AgentCard agent={{ ...BASE_AGENT, is_builtin: true }} onClick={vi.fn()} />);
    expect(screen.getByText('内置')).toBeInTheDocument();
  });

  it('shows disabled state with reduced opacity', () => {
    render(<AgentCard agent={{ ...BASE_AGENT, disabled: true }} onClick={vi.fn()} />);
    const card = screen.getByTestId('agent-card-coder');
    expect(card.className).toContain('opacity-50');
    expect(screen.getByText('已禁用')).toBeInTheDocument();
  });

  it('shows selected state with blue border', () => {
    render(<AgentCard agent={BASE_AGENT} onClick={vi.fn()} selected={true} />);
    const card = screen.getByTestId('agent-card-coder');
    expect(card.className).toContain('border-blue-500');
  });

  it('calls onClick when clicked', () => {
    const onClick = vi.fn();
    render(<AgentCard agent={BASE_AGENT} onClick={onClick} />);
    fireEvent.click(screen.getByTestId('agent-card-coder'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
