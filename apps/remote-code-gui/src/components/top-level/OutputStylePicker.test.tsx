import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { OutputStylePicker } from './OutputStylePicker';

afterEach(() => {
  cleanup();
});

describe('OutputStylePicker', () => {
  const styles = [
    { id: 'concise', name: '简洁' },
    { id: 'verbose', name: '详细' },
  ];

  it('renders style options', () => {
    render(<OutputStylePicker styles={styles} value="concise" onChange={() => {}} />);
    expect(screen.getByTestId('output-style-picker')).toBeInTheDocument();
  });

  it('calls onChange', () => {
    const onChange = vi.fn();
    render(<OutputStylePicker styles={styles} value="concise" onChange={onChange} />);
    fireEvent.change(screen.getByTestId('output-style-select'), { target: { value: 'verbose' } });
    expect(onChange).toHaveBeenCalledWith('verbose');
  });
});
