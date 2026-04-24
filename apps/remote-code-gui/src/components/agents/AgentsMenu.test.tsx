import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AgentsMenu } from './AgentsMenu';

afterEach(() => {
  cleanup();
});

describe('AgentsMenu', () => {
  const agents = [
    { name: 'Agent1', prompt: 'Do stuff' },
    { name: 'Agent2', prompt: 'Do more' },
  ];

  it('renders menu trigger', () => {
    render(<AgentsMenu agents={agents} />);
    expect(screen.getByTestId('agents-menu')).toBeInTheDocument();
    expect(screen.getByTestId('agents-menu-trigger')).toBeInTheDocument();
  });

  it('opens dropdown on click', () => {
    render(<AgentsMenu agents={agents} />);
    fireEvent.click(screen.getByTestId('agents-menu-trigger'));
    expect(screen.getByTestId('agents-menu-dropdown')).toBeInTheDocument();
  });

  it('calls onSelect', () => {
    const onSelect = vi.fn();
    render(<AgentsMenu agents={agents} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('agents-menu-trigger'));
    fireEvent.click(screen.getByTestId('agents-menu-item-Agent1'));
    expect(onSelect).toHaveBeenCalledWith('Agent1');
  });

  it('shows selected agent name', () => {
    render(<AgentsMenu agents={agents} selected="Agent1" />);
    expect(screen.getByText('Agent1')).toBeInTheDocument();
  });
});
