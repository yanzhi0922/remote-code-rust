import { afterEach, describe, it, expect } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { AgentProgressLine } from './AgentProgressLine';

afterEach(() => {
  cleanup();
});

const defaultProps = {
  agentType: 'coder',
  toolUseCount: 5,
  tokens: 1200,
  isLast: false,
  isResolved: false,
  isError: false,
};

describe('AgentProgressLine', () => {
  it('renders agent progress line', () => {
    render(<AgentProgressLine {...defaultProps} />);
    expect(screen.getByTestId('agent-progress-line')).toBeInTheDocument();
  });

  it('shows agent type', () => {
    render(<AgentProgressLine {...defaultProps} />);
    expect(screen.getByText('coder')).toBeInTheDocument();
  });

  it('shows tool use count', () => {
    render(<AgentProgressLine {...defaultProps} />);
    expect(screen.getByText(/5 tool uses/)).toBeInTheDocument();
  });

  it('shows token count', () => {
    render(<AgentProgressLine {...defaultProps} tokens={1200} />);
    expect(screen.getByText(/1.2K tokens/)).toBeInTheDocument();
  });

  it('shows "Initializing…" when not resolved and no lastToolInfo', () => {
    render(<AgentProgressLine {...defaultProps} />);
    expect(screen.getByText('Initializing…')).toBeInTheDocument();
  });

  it('shows lastToolInfo when not resolved', () => {
    render(<AgentProgressLine {...defaultProps} lastToolInfo="Reading file.ts" />);
    expect(screen.getByText('Reading file.ts')).toBeInTheDocument();
  });

  it('shows "Done" when resolved', () => {
    render(<AgentProgressLine {...defaultProps} isResolved={true} />);
    expect(screen.getByText('Done')).toBeInTheDocument();
  });

  it('uses tree char └─ for last item', () => {
    render(<AgentProgressLine {...defaultProps} isLast={true} />);
    expect(screen.getByTestId('agent-progress-line').textContent).toContain('└─');
  });

  it('uses tree char ├─ for non-last item', () => {
    render(<AgentProgressLine {...defaultProps} isLast={false} />);
    expect(screen.getByTestId('agent-progress-line').textContent).toContain('├─');
  });
});
