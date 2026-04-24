import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { StatusIcon } from './StatusIcon';

describe('StatusIcon', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<StatusIcon status="success" />);
    expect(screen.getByTestId('status-icon')).toBeInTheDocument();
  });

  it('renders success icon', () => {
    render(<StatusIcon status="success" />);
    expect(screen.getByTestId('status-icon-success')).toBeInTheDocument();
  });

  it('renders error icon', () => {
    render(<StatusIcon status="error" />);
    expect(screen.getByTestId('status-icon-error')).toBeInTheDocument();
  });

  it('renders warning icon', () => {
    render(<StatusIcon status="warning" />);
    expect(screen.getByTestId('status-icon-warning')).toBeInTheDocument();
  });

  it('renders info icon', () => {
    render(<StatusIcon status="info" />);
    expect(screen.getByTestId('status-icon-info')).toBeInTheDocument();
  });

  it('renders loading icon with spin animation', () => {
    render(<StatusIcon status="loading" />);
    const icon = screen.getByTestId('status-icon-loading');
    expect(icon).toBeInTheDocument();
    expect(icon.getAttribute('class')).toContain('animate-spin');
  });

  it('applies correct color for success', () => {
    render(<StatusIcon status="success" />);
    const icon = screen.getByTestId('status-icon-success');
    expect(icon.getAttribute('class')).toContain('text-green-500');
  });

  it('applies custom className', () => {
    render(<StatusIcon status="info" className="extra" />);
    expect(screen.getByTestId('status-icon').className).toContain('extra');
  });
});
