import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TextInput } from './TextInput';

afterEach(() => {
  cleanup();
});

describe('TextInput', () => {
  it('renders input', () => {
    render(<TextInput value="" onChange={() => {}} />);
    expect(screen.getByTestId('text-input')).toBeInTheDocument();
  });

  it('calls onChange', () => {
    const onChange = vi.fn();
    render(<TextInput value="" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('text-input'), { target: { value: 'hello' } });
    expect(onChange).toHaveBeenCalledWith('hello');
  });

  it('renders multiline', () => {
    render(<TextInput value="" onChange={() => {}} multiline />);
    const el = screen.getByTestId('text-input');
    expect(el.tagName).toBe('TEXTAREA');
  });
});
