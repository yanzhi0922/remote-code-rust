import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { AwsAuthStatusDialog } from './AwsAuthStatusDialog';

afterEach(() => {
  cleanup();
});

describe('AwsAuthStatusDialog', () => {
  it('renders when authenticating', () => {
    render(<AwsAuthStatusDialog status={{ isAuthenticating: true, error: null, output: [] }} />);
    expect(screen.getByTestId('aws-auth-status-dialog')).toBeInTheDocument();
  });

  it('renders when there is an error', () => {
    render(<AwsAuthStatusDialog status={{ isAuthenticating: false, error: 'Auth failed', output: [] }} />);
    expect(screen.getByTestId('aws-auth-status-dialog')).toBeInTheDocument();
    expect(screen.getByText('Auth failed')).toBeInTheDocument();
  });

  it('returns null when no error and not authenticating', () => {
    const { container } = render(
      <AwsAuthStatusDialog status={{ isAuthenticating: false, error: null, output: [] }} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('shows output lines', () => {
    render(
      <AwsAuthStatusDialog
        status={{ isAuthenticating: true, error: null, output: ['line1', 'line2'] }}
      />,
    );
    expect(screen.getByText('line1')).toBeInTheDocument();
    expect(screen.getByText('line2')).toBeInTheDocument();
  });

  it('shows close button when onClose is provided', () => {
    render(
      <AwsAuthStatusDialog
        status={{ isAuthenticating: true, error: null, output: [] }}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByTestId('aws-auth-close')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<AwsAuthStatusDialog status={{ isAuthenticating: true, error: null, output: [] }} />);
    expect(screen.getByText('Cloud Authentication')).toBeInTheDocument();
  });
});
