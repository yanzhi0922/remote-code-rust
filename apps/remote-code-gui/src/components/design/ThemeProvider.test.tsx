import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ThemeProvider, useTheme } from './ThemeProvider';

function ThemeConsumer() {
  const { theme, isDark } = useTheme();
  return (
    <div data-testid="theme-consumer">
      <span data-testid="consumer-theme">{theme}</span>
      <span data-testid="consumer-is-dark">{isDark ? 'true' : 'false'}</span>
    </div>
  );
}

describe('ThemeProvider', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<ThemeProvider>内容</ThemeProvider>);
    expect(screen.getByTestId('theme-provider')).toBeInTheDocument();
  });

  it('renders children', () => {
    render(<ThemeProvider>子内容</ThemeProvider>);
    expect(screen.getByText('子内容')).toBeInTheDocument();
  });

  it('provides light theme by default', () => {
    render(
      <ThemeProvider>
        <ThemeConsumer />
      </ThemeProvider>
    );
    expect(screen.getByTestId('consumer-theme')).toHaveTextContent('light');
    expect(screen.getByTestId('consumer-is-dark')).toHaveTextContent('false');
  });

  it('provides dark theme when specified', () => {
    render(
      <ThemeProvider theme="dark">
        <ThemeConsumer />
      </ThemeProvider>
    );
    expect(screen.getByTestId('consumer-theme')).toHaveTextContent('dark');
    expect(screen.getByTestId('consumer-is-dark')).toHaveTextContent('true');
  });

  it('applies dark background class for dark theme', () => {
    render(<ThemeProvider theme="dark">内容</ThemeProvider>);
    const provider = screen.getByTestId('theme-provider');
    expect(provider.className).toContain('bg-slate-900');
  });

  it('applies light background class for light theme', () => {
    render(<ThemeProvider theme="light">内容</ThemeProvider>);
    const provider = screen.getByTestId('theme-provider');
    expect(provider.className).toContain('bg-white');
  });

  it('useTheme returns default context when used outside provider', () => {
    render(<ThemeConsumer />);
    expect(screen.getByTestId('consumer-theme')).toHaveTextContent('light');
    expect(screen.getByTestId('consumer-is-dark')).toHaveTextContent('false');
  });
});
