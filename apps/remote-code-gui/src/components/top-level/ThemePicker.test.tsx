import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ThemePicker } from './ThemePicker';

afterEach(() => {
  cleanup();
});

describe('ThemePicker', () => {
  it('renders theme options', () => {
    render(<ThemePicker value="light" onChange={() => {}} />);
    expect(screen.getByTestId('theme-picker')).toBeInTheDocument();
    expect(screen.getByText('浅色')).toBeInTheDocument();
    expect(screen.getByText('深色')).toBeInTheDocument();
    expect(screen.getByText('跟随系统')).toBeInTheDocument();
  });

  it('calls onChange', () => {
    const onChange = vi.fn();
    render(<ThemePicker value="light" onChange={onChange} />);
    fireEvent.click(screen.getByTestId('theme-picker-dark'));
    expect(onChange).toHaveBeenCalledWith('dark');
  });
});
