import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { HelpDialog } from './HelpDialog';

afterEach(() => {
  cleanup();
});

describe('HelpDialog', () => {
  it('renders help dialog with tabs', () => {
    render(<HelpDialog onClose={vi.fn()} />);
    expect(screen.getByTestId('help-dialog')).toBeInTheDocument();
    expect(screen.getByTestId('help-tab-general')).toBeInTheDocument();
    expect(screen.getByTestId('help-tab-commands')).toBeInTheDocument();
    expect(screen.getByTestId('help-tab-shortcuts')).toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    render(<HelpDialog onClose={onClose} />);
    fireEvent.click(screen.getByTestId('help-close-btn'));
    expect(onClose).toHaveBeenCalled();
  });

  it('shows general tab by default', () => {
    render(<HelpDialog onClose={vi.fn()} />);
    expect(screen.getByTestId('help-general')).toBeInTheDocument();
  });

  it('switches to commands tab', () => {
    render(<HelpDialog onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId('help-tab-commands'));
    expect(screen.getByTestId('help-commands')).toBeInTheDocument();
    expect(screen.getByText('/help')).toBeInTheDocument();
  });

  it('switches to shortcuts tab', () => {
    render(<HelpDialog onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId('help-tab-shortcuts'));
    expect(screen.getByTestId('help-shortcuts')).toBeInTheDocument();
    expect(screen.getByText('Escape')).toBeInTheDocument();
  });

  it('shows custom commands', () => {
    const customCommands = [{ name: '/custom', description: 'Custom command' }];
    render(<HelpDialog commands={customCommands} onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId('help-tab-commands'));
    expect(screen.getByText('/custom')).toBeInTheDocument();
    expect(screen.getByText('Custom command')).toBeInTheDocument();
  });
});
