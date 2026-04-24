import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { OutputStylePicker } from './OutputStylePicker';

describe('OutputStylePicker', () => {
  afterEach(cleanup);

  it('renders all style options', () => {
    render(<OutputStylePicker value="default" onChange={vi.fn()} />);
    expect(screen.getByText('默认')).toBeInTheDocument();
    expect(screen.getByText('简洁')).toBeInTheDocument();
    expect(screen.getByText('详细')).toBeInTheDocument();
    expect(screen.getByText('JSON')).toBeInTheDocument();
  });

  it('highlights the selected style', () => {
    render(<OutputStylePicker value="concise" onChange={vi.fn()} />);
    const conciseBtn = screen.getByTestId('style-concise');
    expect(conciseBtn.className).toContain('border-blue-500');
  });

  it('calls onChange when a style is clicked', () => {
    const onChange = vi.fn();
    render(<OutputStylePicker value="default" onChange={onChange} />);
    fireEvent.click(screen.getByTestId('style-json'));
    expect(onChange).toHaveBeenCalledWith('json');
  });

  it('renders preview text for each style', () => {
    render(<OutputStylePicker value="default" onChange={vi.fn()} />);
    expect(screen.getByText('标准的输出格式，包含详细说明和代码块。')).toBeInTheDocument();
  });

  it('renders the label', () => {
    render(<OutputStylePicker value="default" onChange={vi.fn()} />);
    expect(screen.getByText('输出风格')).toBeInTheDocument();
  });

  it('does not highlight non-selected styles', () => {
    render(<OutputStylePicker value="default" onChange={vi.fn()} />);
    const jsonBtn = screen.getByTestId('style-json');
    expect(jsonBtn.className).toContain('border-slate-200');
  });
});
