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

  it('provides dark theme when defaultTheme is dark', () => {
    render(
      <ThemeProvider defaultTheme="dark">
        <ThemeConsumer />
      </ThemeProvider>
    );
    expect(screen.getByTestId('consumer-theme')).toHaveTextContent('dark');
    expect(screen.getByTestId('consumer-is-dark')).toHaveTextContent('true');
  });

  it('applies data-theme attribute for dark theme', () => {
    render(<ThemeProvider defaultTheme="dark">内容</ThemeProvider>);
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
  });

  it('applies data-theme attribute for light theme', () => {
    render(<ThemeProvider defaultTheme="light">内容</ThemeProvider>);
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });

  it('useTheme returns default context when used outside provider', () => {
    render(<ThemeConsumer />);
    expect(screen.getByTestId('consumer-theme')).toHaveTextContent('light');
    expect(screen.getByTestId('consumer-is-dark')).toHaveTextContent('false');
  });
});
