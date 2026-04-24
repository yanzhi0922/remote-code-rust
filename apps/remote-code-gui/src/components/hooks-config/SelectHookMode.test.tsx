import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SelectHookMode } from './SelectHookMode';

describe('SelectHookMode', () => {
  afterEach(cleanup);

  it('renders select element', () => {
    render(<SelectHookMode value="block" onChange={vi.fn()} />);
    expect(screen.getByTestId('select-hook-mode')).toBeInTheDocument();
  });

  it('displays current value', () => {
    render(<SelectHookMode value="pass" onChange={vi.fn()} />);
    const select = screen.getByTestId('select-hook-mode') as HTMLSelectElement;
    expect(select.value).toBe('pass');
  });

  it('contains all hook mode options', () => {
    render(<SelectHookMode value="block" onChange={vi.fn()} />);
    expect(screen.getByText('Block')).toBeInTheDocument();
    expect(screen.getByText('Pass')).toBeInTheDocument();
    expect(screen.getByText('Modify')).toBeInTheDocument();
  });

  it('calls onChange when value changes', () => {
    const onChange = vi.fn();
    render(<SelectHookMode value="block" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('select-hook-mode'), {
      target: { value: 'modify' },
    });
    expect(onChange).toHaveBeenCalledWith('modify');
  });

  it('applies custom className', () => {
    render(
      <SelectHookMode value="block" onChange={vi.fn()} className="custom" />,
    );
    expect(screen.getByTestId('select-hook-mode').className).toContain('custom');
  });

  it('has border styling', () => {
    render(<SelectHookMode value="block" onChange={vi.fn()} />);
    expect(screen.getByTestId('select-hook-mode').className).toContain('border-slate-200');
  });
});
