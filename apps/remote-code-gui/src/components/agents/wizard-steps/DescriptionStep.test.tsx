import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DescriptionStep } from './DescriptionStep';

describe('DescriptionStep', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<DescriptionStep value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('wizard-description-step')).toBeInTheDocument();
  });

  it('renders textarea with current value', () => {
    render(<DescriptionStep value="测试描述" onChange={vi.fn()} />);
    const input = screen.getByTestId('description-input') as HTMLTextAreaElement;
    expect(input.value).toBe('测试描述');
  });

  it('calls onChange when text is entered', () => {
    const onChange = vi.fn();
    render(<DescriptionStep value="" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('description-input'), { target: { value: '新描述' } });
    expect(onChange).toHaveBeenCalledWith('新描述');
  });

  it('shows character count', () => {
    render(<DescriptionStep value="abc" onChange={vi.fn()} />);
    expect(screen.getByText('3 字符')).toBeInTheDocument();
  });

  it('shows zero character count for empty value', () => {
    render(<DescriptionStep value="" onChange={vi.fn()} />);
    expect(screen.getByText('0 字符')).toBeInTheDocument();
  });

  it('renders placeholder text', () => {
    render(<DescriptionStep value="" onChange={vi.fn()} />);
    const input = screen.getByTestId('description-input') as HTMLTextAreaElement;
    expect(input.placeholder).toContain('例如');
  });

  it('applies custom className', () => {
    render(<DescriptionStep value="" onChange={vi.fn()} className="custom-class" />);
    expect(screen.getByTestId('wizard-description-step').className).toContain('custom-class');
  });
});
