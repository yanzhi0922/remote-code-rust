import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { ConsoleOAuthDialog } from './ConsoleOAuthDialog';

afterEach(() => {
  cleanup();
});

describe('ConsoleOAuthDialog', () => {
  it('renders with data-testid', () => {
    render(<ConsoleOAuthDialog onDone={vi.fn()} />);
    expect(screen.getByTestId('console-oauth-dialog')).toBeInTheDocument();
  });

  it('shows sign in title by default', () => {
    render(<ConsoleOAuthDialog onDone={vi.fn()} />);
    expect(screen.getByText('Sign In')).toBeInTheDocument();
  });

  it('shows setup token title in setup-token mode', () => {
    render(<ConsoleOAuthDialog onDone={vi.fn()} mode="setup-token" />);
    expect(screen.getByText('Set Up API Token')).toBeInTheDocument();
  });

  it('shows starting message when provided', () => {
    render(<ConsoleOAuthDialog onDone={vi.fn()} startingMessage="Welcome back" />);
    expect(screen.getByText('Welcome back')).toBeInTheDocument();
  });

  it('calls onDone when cancel is clicked', () => {
    const onDone = vi.fn();
    render(<ConsoleOAuthDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('console-oauth-cancel'));
    expect(onDone).toHaveBeenCalledOnce();
  });

  it('calls onDone when close is clicked', () => {
    const onDone = vi.fn();
    render(<ConsoleOAuthDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('console-oauth-close'));
    expect(onDone).toHaveBeenCalledOnce();
  });

  it('shows login method options in idle state', () => {
    render(<ConsoleOAuthDialog onDone={vi.fn()} />);
    expect(screen.getByTestId('console-oauth-claudeai')).toBeInTheDocument();
    expect(screen.getByTestId('console-oauth-console')).toBeInTheDocument();
  });
});
