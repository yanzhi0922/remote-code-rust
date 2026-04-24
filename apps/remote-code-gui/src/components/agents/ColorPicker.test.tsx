import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ColorPicker } from './ColorPicker';

describe('ColorPicker', () => {
  afterEach(cleanup);

  it('renders all 9 preset color buttons', () => {
    render(<ColorPicker value="#3b82f6" onChange={vi.fn()} />);
    const buttons = screen.getAllByRole('button', { name: /^选择.+色$/ });
    expect(buttons).toHaveLength(9);
  });

  it('highlights the currently selected color', () => {
    render(<ColorPicker value="#3b82f6" onChange={vi.fn()} />);
    const blueButton = screen.getByLabelText('选择蓝色');
    expect(blueButton.className).toContain('ring-2');
  });

  it('calls onChange when a preset color is clicked', () => {
    const onChange = vi.fn();
    render(<ColorPicker value="#3b82f6" onChange={onChange} />);
    fireEvent.click(screen.getByLabelText('选择红色'));
    expect(onChange).toHaveBeenCalledWith('#ef4444');
  });

  it('shows current color preview', () => {
    render(<ColorPicker value="#22c55e" onChange={vi.fn()} />);
    expect(screen.getByText('#22c55e')).toBeInTheDocument();
  });

  it('applies custom hex color via input', () => {
    const onChange = vi.fn();
    render(<ColorPicker value="#3b82f6" onChange={onChange} />);
    const input = screen.getByLabelText('自定义颜色输入');
    fireEvent.change(input, { target: { value: '#ff5500' } });
    fireEvent.click(screen.getByRole('button', { name: '应用' }));
    expect(onChange).toHaveBeenCalledWith('#ff5500');
  });

  it('applies custom hex color on Enter key', () => {
    const onChange = vi.fn();
    render(<ColorPicker value="#3b82f6" onChange={onChange} />);
    const input = screen.getByLabelText('自定义颜色输入');
    fireEvent.change(input, { target: { value: '#aabbcc' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onChange).toHaveBeenCalledWith('#aabbcc');
  });

  it('does not call onChange for empty custom input', () => {
    const onChange = vi.fn();
    render(<ColorPicker value="#3b82f6" onChange={onChange} />);
    fireEvent.click(screen.getByRole('button', { name: '应用' }));
    expect(onChange).not.toHaveBeenCalled();
  });
});
