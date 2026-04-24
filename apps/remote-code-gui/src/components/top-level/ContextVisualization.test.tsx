import { afterEach, describe, it, expect } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { ContextVisualization } from './ContextVisualization';

afterEach(() => {
  cleanup();
});

const defaultProps = {
  categories: [
    { name: 'System Prompt', tokens: 5000, color: 'bg-blue-500' },
    { name: 'Messages', tokens: 12000, color: 'bg-green-500' },
  ],
  totalTokens: 17000,
  maxTokens: 200000,
  percentage: 8,
  model: 'claude-sonnet',
};

describe('ContextVisualization', () => {
  it('renders context usage bar', () => {
    render(<ContextVisualization {...defaultProps} />);
    expect(screen.getByTestId('context-visualization')).toBeInTheDocument();
    expect(screen.getByTestId('context-bar')).toBeInTheDocument();
  });

  it('shows model and token count', () => {
    render(<ContextVisualization {...defaultProps} />);
    expect(screen.getByText(/claude-sonnet/)).toBeInTheDocument();
    expect(screen.getByText(/17\.0K/)).toBeInTheDocument();
    expect(screen.getByText(/200\.0K/)).toBeInTheDocument();
  });

  it('renders category breakdown', () => {
    render(<ContextVisualization {...defaultProps} />);
    expect(screen.getByTestId('context-categories')).toBeInTheDocument();
    expect(screen.getByText('System Prompt')).toBeInTheDocument();
    expect(screen.getByText('Messages')).toBeInTheDocument();
  });

  it('shows green bar when usage is low', () => {
    render(<ContextVisualization {...defaultProps} percentage={8} />);
    const fill = screen.getByTestId('context-bar-fill');
    expect(fill.classList.contains('bg-green-500')).toBe(true);
  });

  it('shows red bar when usage is high', () => {
    render(<ContextVisualization {...defaultProps} percentage={95} />);
    const fill = screen.getByTestId('context-bar-fill');
    expect(fill.classList.contains('bg-red-500')).toBe(true);
  });

  it('shows yellow bar when usage is moderate', () => {
    render(<ContextVisualization {...defaultProps} percentage={75} />);
    const fill = screen.getByTestId('context-bar-fill');
    expect(fill.classList.contains('bg-yellow-500')).toBe(true);
  });
});
