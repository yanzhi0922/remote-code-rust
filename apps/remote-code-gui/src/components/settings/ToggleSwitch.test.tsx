import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ToggleSwitch } from './ToggleSwitch';

describe('ToggleSwitch', () => {
  afterEach(cleanup);

  it('renders label text', () => {
    render(<ToggleSwitch checked={false} onChange={vi.fn()} label="Verbose" />);
    expect(screen.getByText('Verbose')).toBeInTheDocument();
  });

  it('renders description when provided', () => {
    render(
      <ToggleSwitch checked={false} onChange={vi.fn()} label="Verbose" description="Enable verbose logging" />,
    );
    expect(screen.getByText('Enable verbose logging')).toBeInTheDocument();
  });

  it('does not render description when not provided', () => {
    render(<ToggleSwitch checked={false} onChange={vi.fn()} label="Verbose" />);
    expect(screen.queryByText('Enable verbose logging')).toBeNull();
  });

  it('calls onChange with true when clicked while unchecked', () => {
    const onChange = vi.fn();
    render(<ToggleSwitch checked={false} onChange={onChange} label="Verbose" />);
    fireEvent.click(screen.getByRole('switch'));
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('calls onChange with false when clicked while checked', () => {
    const onChange = vi.fn();
    render(<ToggleSwitch checked={true} onChange={onChange} label="Verbose" />);
    fireEvent.click(screen.getByRole('switch'));
    expect(onChange).toHaveBeenCalledWith(false);
  });

  it('reflects checked state via aria-checked', () => {
    render(<ToggleSwitch checked={true} onChange={vi.fn()} label="Verbose" />);
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
  });

  it('is disabled when disabled prop is true', () => {
    render(<ToggleSwitch checked={false} onChange={vi.fn()} label="Verbose" disabled={true} />);
    expect(screen.getByRole('switch')).toBeDisabled();
  });

  it('does not call onChange when disabled', () => {
    const onChange = vi.fn();
    render(<ToggleSwitch checked={false} onChange={onChange} label="Verbose" disabled={true} />);
    fireEvent.click(screen.getByRole('switch'));
    expect(onChange).not.toHaveBeenCalled();
  });
});
