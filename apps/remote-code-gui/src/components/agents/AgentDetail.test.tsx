import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AgentDetail } from './AgentDetail';

afterEach(() => {
  cleanup();
});

describe('AgentDetail', () => {
  const agent = {
    name: 'TestAgent',
    description: 'A test agent',
    model: 'gpt-4',
    tools: ['bash', 'edit'],
  };

  it('renders agent info', () => {
    render(<AgentDetail agent={agent} />);
    expect(screen.getByTestId('agent-detail')).toBeInTheDocument();
    expect(screen.getByText('TestAgent')).toBeInTheDocument();
    expect(screen.getByText('A test agent')).toBeInTheDocument();
  });

  it('calls onEdit', () => {
    const onEdit = vi.fn();
    render(<AgentDetail agent={agent} onEdit={onEdit} />);
    fireEvent.click(screen.getByTestId('agent-detail-edit'));
    expect(onEdit).toHaveBeenCalled();
  });

  it('calls onDelete', () => {
    const onDelete = vi.fn();
    render(<AgentDetail agent={agent} onDelete={onDelete} />);
    fireEvent.click(screen.getByTestId('agent-detail-delete'));
    expect(onDelete).toHaveBeenCalled();
  });
});
