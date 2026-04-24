import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AgentsList } from './AgentsList';

afterEach(() => {
  cleanup();
});

describe('AgentsList', () => {
  const agents = [
    { name: 'Agent1', prompt: 'Do stuff' },
    { name: 'Agent2', prompt: 'Do more' },
  ];

  it('renders agents', () => {
    render(<AgentsList agents={agents} />);
    expect(screen.getByTestId('agents-list')).toBeInTheDocument();
    expect(screen.getByText('Agent1')).toBeInTheDocument();
    expect(screen.getByText('Agent2')).toBeInTheDocument();
  });

  it('shows empty state', () => {
    render(<AgentsList agents={[]} />);
    expect(screen.getByTestId('agents-list-empty')).toHaveTextContent('暂无Agent');
  });

  it('calls onSelect', () => {
    const onSelect = vi.fn();
    render(<AgentsList agents={agents} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('agents-list-item-0'));
    expect(onSelect).toHaveBeenCalledWith(0);
  });

  it('calls onAdd', () => {
    const onAdd = vi.fn();
    render(<AgentsList agents={agents} onAdd={onAdd} />);
    fireEvent.click(screen.getByTestId('agents-list-add'));
    expect(onAdd).toHaveBeenCalled();
  });
});
