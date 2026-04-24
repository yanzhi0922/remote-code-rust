import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { InvalidConfigDialog } from './InvalidConfigDialog';

afterEach(() => {
  cleanup();
});

describe('InvalidConfigDialog', () => {
  it('renders with data-testid', () => {
    render(
      <InvalidConfigDialog
        filePath="/path/to/config.json"
        errorDescription="Invalid JSON"
        onExit={vi.fn()}
        onReset={vi.fn()}
      />,
    );
    expect(screen.getByTestId('invalid-config-dialog')).toBeInTheDocument();
  });

  it('shows the file path', () => {
    render(
      <InvalidConfigDialog
        filePath="/path/to/config.json"
        errorDescription="Invalid JSON"
        onExit={vi.fn()}
        onReset={vi.fn()}
      />,
    );
    expect(screen.getByText('/path/to/config.json')).toBeInTheDocument();
  });

  it('shows the error description', () => {
    render(
      <InvalidConfigDialog
        filePath="/path/to/config.json"
        errorDescription="Unexpected token"
        onExit={vi.fn()}
        onReset={vi.fn()}
      />,
    );
    expect(screen.getByText('Unexpected token')).toBeInTheDocument();
  });

  it('calls onExit when exit button is clicked', () => {
    const onExit = vi.fn();
    render(
      <InvalidConfigDialog
        filePath="/path/to/config.json"
        errorDescription="Invalid JSON"
        onExit={onExit}
        onReset={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('invalid-config-exit'));
    expect(onExit).toHaveBeenCalledOnce();
  });

  it('calls onReset when reset button is clicked', () => {
    const onReset = vi.fn();
    render(
      <InvalidConfigDialog
        filePath="/path/to/config.json"
        errorDescription="Invalid JSON"
        onExit={vi.fn()}
        onReset={onReset}
      />,
    );
    fireEvent.click(screen.getByTestId('invalid-config-reset'));
    expect(onReset).toHaveBeenCalledOnce();
  });

  it('calls onExit when close is clicked', () => {
    const onExit = vi.fn();
    render(
      <InvalidConfigDialog
        filePath="/path/to/config.json"
        errorDescription="Invalid JSON"
        onExit={onExit}
        onReset={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('invalid-config-close'));
    expect(onExit).toHaveBeenCalledOnce();
  });
});
