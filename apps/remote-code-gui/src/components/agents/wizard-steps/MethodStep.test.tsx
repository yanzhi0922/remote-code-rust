import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MethodStep } from './MethodStep';

describe('MethodStep', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<MethodStep value="manual" onChange={vi.fn()} />);
    expect(screen.getByTestId('wizard-method-step')).toBeInTheDocument();
  });

  it('renders both method options', () => {
    render(<MethodStep value="manual" onChange={vi.fn()} />);
    expect(screen.getByTestId('method-option-manual')).toBeInTheDocument();
    expect(screen.getByTestId('method-option-generate')).toBeInTheDocument();
  });

  it('shows selected state for current value', () => {
    render(<MethodStep value="generate" onChange={vi.fn()} />);
    const generateBtn = screen.getByTestId('method-option-generate');
    expect(generateBtn.className).toContain('border-blue-500');
  });

  it('does not show selected state for non-selected method', () => {
    render(<MethodStep value="manual" onChange={vi.fn()} />);
    const generateBtn = screen.getByTestId('method-option-generate');
    expect(generateBtn.className).not.toContain('border-blue-500');
  });

  it('calls onChange when method option is clicked', () => {
    const onChange = vi.fn();
    render(<MethodStep value="manual" onChange={onChange} />);
    fireEvent.click(screen.getByTestId('method-option-generate'));
    expect(onChange).toHaveBeenCalledWith('generate');
  });

  it('renders method labels', () => {
    render(<MethodStep value="manual" onChange={vi.fn()} />);
    expect(screen.getByText('手动创建')).toBeInTheDocument();
    expect(screen.getByText('AI 生成')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(<MethodStep value="manual" onChange={vi.fn()} className="extra" />);
    expect(screen.getByTestId('wizard-method-step').className).toContain('extra');
  });
});
