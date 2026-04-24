import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { SessionPreview } from './SessionPreview';

afterEach(() => {
  cleanup();
});

const defaultProps = {
  id: 'session-1',
  title: 'Code Review Session',
  messageCount: 42,
  modified: '2 hours ago',
};

describe('SessionPreview', () => {
  it('renders session preview card', () => {
    render(<SessionPreview {...defaultProps} />);
    expect(screen.getByTestId('session-preview')).toBeInTheDocument();
  });

  it('shows session title', () => {
    render(<SessionPreview {...defaultProps} />);
    expect(screen.getByText('Code Review Session')).toBeInTheDocument();
  });

  it('shows message count', () => {
    render(<SessionPreview {...defaultProps} />);
    expect(screen.getByText('42 messages')).toBeInTheDocument();
  });

  it('shows modified time', () => {
    render(<SessionPreview {...defaultProps} />);
    expect(screen.getByText('2 hours ago')).toBeInTheDocument();
  });

  it('shows git branch when provided', () => {
    render(<SessionPreview {...defaultProps} gitBranch="feature/auth" />);
    expect(screen.getByText('feature/auth')).toBeInTheDocument();
  });

  it('does not show git branch when not provided', () => {
    render(<SessionPreview {...defaultProps} />);
    expect(screen.queryByText('feature/auth')).not.toBeInTheDocument();
  });

  it('shows loading state', () => {
    render(<SessionPreview {...defaultProps} isLoading />);
    expect(screen.getByText('Loading session…')).toBeInTheDocument();
  });

  it('calls onSelect when select button is clicked', () => {
    const onSelect = vi.fn();
    render(<SessionPreview {...defaultProps} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('session-preview-select'));
    expect(onSelect).toHaveBeenCalled();
  });

  it('calls onExit when cancel button is clicked', () => {
    const onExit = vi.fn();
    render(<SessionPreview {...defaultProps} onExit={onExit} />);
    fireEvent.click(screen.getByTestId('session-preview-cancel'));
    expect(onExit).toHaveBeenCalled();
  });
});
