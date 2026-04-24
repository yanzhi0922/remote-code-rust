import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SelectMatcherMode } from './SelectMatcherMode';

describe('SelectMatcherMode', () => {
  afterEach(cleanup);

  it('renders select element', () => {
    render(<SelectMatcherMode value="regex" onChange={vi.fn()} />);
    expect(screen.getByTestId('select-matcher-mode')).toBeInTheDocument();
  });

  it('displays current value', () => {
    render(<SelectMatcherMode value="glob" onChange={vi.fn()} />);
    const select = screen.getByTestId('select-matcher-mode') as HTMLSelectElement;
    expect(select.value).toBe('glob');
  });

  it('contains all matcher mode options', () => {
    render(<SelectMatcherMode value="regex" onChange={vi.fn()} />);
    expect(screen.getByText('Regex')).toBeInTheDocument();
    expect(screen.getByText('Glob')).toBeInTheDocument();
    expect(screen.getByText('Exact')).toBeInTheDocument();
  });

  it('calls onChange when value changes', () => {
    const onChange = vi.fn();
    render(<SelectMatcherMode value="regex" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('select-matcher-mode'), {
      target: { value: 'exact' },
    });
    expect(onChange).toHaveBeenCalledWith('exact');
  });

  it('applies custom className', () => {
    render(
      <SelectMatcherMode value="regex" onChange={vi.fn()} className="custom" />,
    );
    expect(screen.getByTestId('select-matcher-mode').className).toContain('custom');
  });

  it('has focus ring styles', () => {
    render(<SelectMatcherMode value="regex" onChange={vi.fn()} />);
    expect(screen.getByTestId('select-matcher-mode').className).toContain('focus:ring-blue-500');
  });
});
