import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Textarea } from './Textarea';

afterEach(() => {
  cleanup();
});

describe('Textarea', () => {
  it('renders with initial value', () => {
    render(<Textarea value="Hello" onChange={vi.fn()} />);
    expect(screen.getByTestId('textarea')).toHaveValue('Hello');
  });

  it('calls onChange when value changes', () => {
    const onChange = vi.fn();
    render(<Textarea value="" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('textarea'), { target: { value: 'new text' } });
    expect(onChange).toHaveBeenCalledWith('new text');
  });

  it('renders placeholder', () => {
    render(<Textarea value="" onChange={vi.fn()} placeholder="Enter text..." />);
    expect(screen.getByTestId('textarea')).toHaveAttribute('placeholder', 'Enter text...');
  });

  it('sets default rows to 3', () => {
    render(<Textarea value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('textarea')).toHaveAttribute('rows', '3');
  });

  it('sets custom rows', () => {
    render(<Textarea value="" onChange={vi.fn()} rows={5} />);
    expect(screen.getByTestId('textarea')).toHaveAttribute('rows', '5');
  });

  it('is disabled when disabled prop is true', () => {
    render(<Textarea value="" onChange={vi.fn()} disabled />);
    expect(screen.getByTestId('textarea')).toBeDisabled();
  });

  it('applies disabled styles', () => {
    render(<Textarea value="" onChange={vi.fn()} disabled />);
    const textarea = screen.getByTestId('textarea');
    expect(textarea.className).toContain('cursor-not-allowed');
    expect(textarea.className).toContain('bg-slate-50');
  });

  it('applies resize-none class', () => {
    render(<Textarea value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('textarea').className).toContain('resize-none');
  });

  it('applies rounded-xl class', () => {
    render(<Textarea value="" onChange={vi.fn()} />);
    expect(screen.getByTestId('textarea').className).toContain('rounded-xl');
  });

  it('merges custom className', () => {
    render(<Textarea value="" onChange={vi.fn()} className="my-textarea" />);
    expect(screen.getByTestId('textarea').className).toContain('my-textarea');
  });
});
