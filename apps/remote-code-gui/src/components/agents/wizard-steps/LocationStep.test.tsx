import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { LocationStep } from './LocationStep';

describe('LocationStep', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<LocationStep value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('wizard-location-step')).toBeInTheDocument();
  });

  it('renders input with current value', () => {
    render(<LocationStep value="/path/to/agents" onChange={vi.fn()} />);
    const input = screen.getByTestId('location-input') as HTMLInputElement;
    expect(input.value).toBe('/path/to/agents');
  });

  it('calls onChange when text is entered', () => {
    const onChange = vi.fn();
    render(<LocationStep value="" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('location-input'), { target: { value: '/new/path' } });
    expect(onChange).toHaveBeenCalledWith('/new/path');
  });

  it('shows path preview when value is set', () => {
    render(<LocationStep value="/my/agents" onChange={vi.fn()} />);
    expect(screen.getByTestId('location-preview')).toHaveTextContent('/my/agents');
  });

  it('does not show preview when value is empty', () => {
    render(<LocationStep value="" onChange={vi.fn()} />);
    expect(screen.queryByTestId('location-preview')).not.toBeInTheDocument();
  });

  it('renders browse button', () => {
    render(<LocationStep value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('location-browse')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(<LocationStep value="" onChange={vi.fn()} className="custom" />);
    expect(screen.getByTestId('wizard-location-step').className).toContain('custom');
  });
});
