import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ThemePicker } from './ThemePicker';

describe('ThemePicker', () => {
  afterEach(cleanup);

  it('renders all theme options', () => {
    render(<ThemePicker value="light" onChange={vi.fn()} />);
    expect(screen.getByText('浅色')).toBeInTheDocument();
    expect(screen.getByText('深色')).toBeInTheDocument();
    expect(screen.getByText('跟随系统')).toBeInTheDocument();
  });

  it('highlights the selected theme', () => {
    render(<ThemePicker value="dark" onChange={vi.fn()} />);
    const darkBtn = screen.getByTestId('theme-dark');
    expect(darkBtn.className).toContain('border-blue-500');
  });

  it('calls onChange when a theme is clicked', () => {
    const onChange = vi.fn();
    render(<ThemePicker value="light" onChange={onChange} />);
    fireEvent.click(screen.getByTestId('theme-dark'));
    expect(onChange).toHaveBeenCalledWith('dark');
  });

  it('renders color preview swatches', () => {
    render(<ThemePicker value="light" onChange={vi.fn()} />);
    const lightBtn = screen.getByTestId('theme-light');
    const swatches = lightBtn.querySelectorAll('span.rounded-full');
    expect(swatches.length).toBe(3);
  });

  it('renders the label', () => {
    render(<ThemePicker value="light" onChange={vi.fn()} />);
    expect(screen.getByText('主题')).toBeInTheDocument();
  });

  it('applies correct icon color for selected theme', () => {
    render(<ThemePicker value="system" onChange={vi.fn()} />);
    const systemBtn = screen.getByTestId('theme-system');
    const icon = systemBtn.querySelector('svg');
    expect(icon?.className.baseVal).toContain('text-blue-600');
  });
});
