import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { TeleportProgressDialog } from './TeleportProgressDialog';

afterEach(() => {
  cleanup();
});

describe('TeleportProgressDialog', () => {
  it('renders with data-testid', () => {
    render(<TeleportProgressDialog currentStep="validating" />);
    expect(screen.getByTestId('teleport-progress-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<TeleportProgressDialog currentStep="validating" />);
    expect(screen.getByText('Teleporting session…')).toBeInTheDocument();
  });

  it('shows session ID when provided', () => {
    render(<TeleportProgressDialog currentStep="validating" sessionId="sess-123" />);
    expect(screen.getByText('sess-123')).toBeInTheDocument();
  });

  it('shows all step labels', () => {
    render(<TeleportProgressDialog currentStep="fetching_logs" />);
    expect(screen.getByText('Validating session')).toBeInTheDocument();
    expect(screen.getByText('Fetching session logs')).toBeInTheDocument();
    expect(screen.getByText('Getting branch info')).toBeInTheDocument();
    expect(screen.getByText('Checking out branch')).toBeInTheDocument();
  });

  it('marks completed steps with check icon', () => {
    render(<TeleportProgressDialog currentStep="fetching_branch" />);
    const completedText = screen.getByText('Validating session');
    expect(completedText.className).toContain('green');
  });

  it('marks current step as active', () => {
    render(<TeleportProgressDialog currentStep="fetching_logs" />);
    const currentText = screen.getByText('Fetching session logs');
    expect(currentText.className).toContain('font-semibold');
  });
});
