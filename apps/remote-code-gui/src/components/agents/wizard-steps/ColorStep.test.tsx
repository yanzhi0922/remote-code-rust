import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ColorStep } from './ColorStep';

describe('ColorStep', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<ColorStep value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('wizard-color-step')).toBeInTheDocument();
  });

  it('renders color grid with preset colors', () => {
    render(<ColorStep value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('color-grid')).toBeInTheDocument();
    expect(screen.getByTestId('color-preset-ef4444')).toBeInTheDocument();
  });

  it('calls onChange when a preset color is clicked', () => {
    const onChange = vi.fn();
    render(<ColorStep value="" onChange={onChange} />);
    fireEvent.click(screen.getByTestId('color-preset-3b82f6'));
    expect(onChange).toHaveBeenCalledWith('#3b82f6');
  });

  it('shows selected state for current value', () => {
    render(<ColorStep value="#ef4444" onChange={vi.fn()} />);
    const selectedBtn = screen.getByTestId('color-preset-ef4444');
    expect(selectedBtn.className).toContain('border-slate-800');
  });

  it('shows color value text', () => {
    render(<ColorStep value="#22c55e" onChange={vi.fn()} />);
    expect(screen.getByTestId('color-value')).toHaveTextContent('#22c55e');
  });

  it('shows preview when color is selected', () => {
    render(<ColorStep value="#3b82f6" onChange={vi.fn()} />);
    expect(screen.getByTestId('color-preview')).toBeInTheDocument();
  });

  it('does not show preview when no color is selected', () => {
    render(<ColorStep value="" onChange={vi.fn()} />);
    expect(screen.queryByTestId('color-preview')).not.toBeInTheDocument();
  });

  it('renders custom color input', () => {
    render(<ColorStep value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('custom-color-input')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(<ColorStep value="" onChange={vi.fn()} className="test-cls" />);
    expect(screen.getByTestId('wizard-color-step').className).toContain('test-cls');
  });
});
