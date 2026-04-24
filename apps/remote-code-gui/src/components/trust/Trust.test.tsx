import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { TrustDialog } from './TrustDialog';

afterEach(() => {
  cleanup();
});

const mockWarnings = [
  { type: 'mcp', label: 'MCP Servers', description: 'Project has MCP server configurations' },
  { type: 'hooks', label: 'Hooks', description: 'Project has hooks configured' },
];

describe('TrustDialog', () => {
  it('renders trust dialog', () => {
    render(<TrustDialog onAccept={vi.fn()} onDecline={vi.fn()} />);
    expect(screen.getByTestId('trust-dialog')).toBeInTheDocument();
    expect(screen.getByText('Trust This Project?')).toBeInTheDocument();
  });

  it('shows project name', () => {
    render(<TrustDialog onAccept={vi.fn()} onDecline={vi.fn()} projectName="my-project" />);
    expect(screen.getByText('my-project')).toBeInTheDocument();
  });

  it('shows warnings', () => {
    render(<TrustDialog warnings={mockWarnings} onAccept={vi.fn()} onDecline={vi.fn()} />);
    expect(screen.getByTestId('trust-warning-mcp')).toBeInTheDocument();
    expect(screen.getByTestId('trust-warning-hooks')).toBeInTheDocument();
    expect(screen.getByText('MCP Servers')).toBeInTheDocument();
  });

  it('calls onAccept when accept button clicked', () => {
    const onAccept = vi.fn();
    render(<TrustDialog onAccept={onAccept} onDecline={vi.fn()} />);
    fireEvent.click(screen.getByTestId('trust-accept-btn'));
    expect(onAccept).toHaveBeenCalled();
    expect(screen.getByTestId('trust-dialog-accepted')).toBeInTheDocument();
  });

  it('calls onDecline when decline button clicked', () => {
    const onDecline = vi.fn();
    render(<TrustDialog onAccept={vi.fn()} onDecline={onDecline} />);
    fireEvent.click(screen.getByTestId('trust-decline-btn'));
    expect(onDecline).toHaveBeenCalled();
  });

  it('shows accepted state after accept', () => {
    render(<TrustDialog onAccept={vi.fn()} onDecline={vi.fn()} />);
    fireEvent.click(screen.getByTestId('trust-accept-btn'));
    expect(screen.getByText('✓ Trust accepted. Session will continue.')).toBeInTheDocument();
  });
});
