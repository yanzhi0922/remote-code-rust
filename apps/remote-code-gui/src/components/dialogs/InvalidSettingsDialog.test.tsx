import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { InvalidSettingsDialog } from './InvalidSettingsDialog';

afterEach(() => {
  cleanup();
});

describe('InvalidSettingsDialog', () => {
  const errors = [
    { file: 'settings.json', message: 'Invalid type for key "theme"' },
    { file: 'local.json', message: 'Missing required field' },
  ];

  it('renders with data-testid', () => {
    render(<InvalidSettingsDialog settingsErrors={errors} onContinue={vi.fn()} onExit={vi.fn()} />);
    expect(screen.getByTestId('invalid-settings-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<InvalidSettingsDialog settingsErrors={errors} onContinue={vi.fn()} onExit={vi.fn()} />);
    expect(screen.getByText('Settings Error')).toBeInTheDocument();
  });

  it('shows error details', () => {
    render(<InvalidSettingsDialog settingsErrors={errors} onContinue={vi.fn()} onExit={vi.fn()} />);
    expect(screen.getByText('settings.json')).toBeInTheDocument();
    expect(screen.getByText(/Invalid type/)).toBeInTheDocument();
  });

  it('calls onExit when exit button is clicked', () => {
    const onExit = vi.fn();
    render(<InvalidSettingsDialog settingsErrors={errors} onContinue={vi.fn()} onExit={onExit} />);
    fireEvent.click(screen.getByTestId('invalid-settings-exit'));
    expect(onExit).toHaveBeenCalledOnce();
  });

  it('calls onContinue when continue button is clicked', () => {
    const onContinue = vi.fn();
    render(<InvalidSettingsDialog settingsErrors={errors} onContinue={onContinue} onExit={vi.fn()} />);
    fireEvent.click(screen.getByTestId('invalid-settings-continue'));
    expect(onContinue).toHaveBeenCalledOnce();
  });

  it('shows skip hint', () => {
    render(<InvalidSettingsDialog settingsErrors={errors} onContinue={vi.fn()} onExit={vi.fn()} />);
    expect(screen.getByText(/skipped entirely/)).toBeInTheDocument();
  });
});
