import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { TeammateViewHeader } from './TeammateViewHeader';

afterEach(() => {
  cleanup();
});

describe('TeammateViewHeader', () => {
  it('renders header with agent name', () => {
    render(<TeammateViewHeader agentName="researcher" />);
    expect(screen.getByTestId('teammate-view-header')).toBeInTheDocument();
    expect(screen.getByTestId('teammate-name')).toHaveTextContent('@researcher');
  });

  it('shows "Viewing" label', () => {
    render(<TeammateViewHeader agentName="researcher" />);
    expect(screen.getByText('Viewing')).toBeInTheDocument();
  });

  it('shows prompt when provided', () => {
    render(<TeammateViewHeader agentName="researcher" prompt="Analyze the codebase" />);
    expect(screen.getByTestId('teammate-prompt')).toHaveTextContent('Analyze the codebase');
  });

  it('does not show prompt section when not provided', () => {
    render(<TeammateViewHeader agentName="researcher" />);
    expect(screen.queryByTestId('teammate-prompt')).not.toBeInTheDocument();
  });

  it('calls onExit when exit button is clicked', () => {
    const onExit = vi.fn();
    render(<TeammateViewHeader agentName="researcher" onExit={onExit} />);
    fireEvent.click(screen.getByTestId('teammate-exit'));
    expect(onExit).toHaveBeenCalled();
  });

  it('applies custom color', () => {
    render(<TeammateViewHeader agentName="researcher" color="text-red-500" />);
    const nameEl = screen.getByTestId('teammate-name');
    expect(nameEl.classList.contains('text-red-500')).toBe(true);
  });
});
