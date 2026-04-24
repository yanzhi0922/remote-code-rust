import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { DesktopHandoffDialog } from './DesktopHandoffDialog';

afterEach(() => {
  cleanup();
});

describe('DesktopHandoffDialog', () => {
  it('renders with data-testid', () => {
    render(<DesktopHandoffDialog onDone={vi.fn()} />);
    expect(screen.getByTestId('desktop-handoff-dialog')).toBeInTheDocument();
  });

  it('shows title', () => {
    render(<DesktopHandoffDialog onDone={vi.fn()} />);
    expect(screen.getByText('Desktop Handoff')).toBeInTheDocument();
  });

  it('shows checking state initially', () => {
    render(<DesktopHandoffDialog onDone={vi.fn()} />);
    expect(screen.getByText('Checking desktop installation…')).toBeInTheDocument();
  });

  it('transitions to prompt-download when simulate is clicked', () => {
    render(<DesktopHandoffDialog onDone={vi.fn()} />);
    fireEvent.click(screen.getByTestId('desktop-handoff-simulate'));
    expect(screen.getByText(/Claude Desktop is not installed/)).toBeInTheDocument();
  });

  it('calls onDone when download yes is clicked', () => {
    const onDone = vi.fn();
    render(<DesktopHandoffDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('desktop-handoff-simulate'));
    fireEvent.click(screen.getByTestId('desktop-handoff-download-yes'));
    expect(onDone).toHaveBeenCalled();
  });

  it('calls onDone when download no is clicked', () => {
    const onDone = vi.fn();
    render(<DesktopHandoffDialog onDone={onDone} />);
    fireEvent.click(screen.getByTestId('desktop-handoff-simulate'));
    fireEvent.click(screen.getByTestId('desktop-handoff-download-no'));
    expect(onDone).toHaveBeenCalledWith('The desktop app is required for /desktop.');
  });
});
