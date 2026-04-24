import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ThemedBox } from './ThemedBox';

describe('ThemedBox', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<ThemedBox>内容</ThemedBox>);
    expect(screen.getByTestId('themed-box')).toBeInTheDocument();
  });

  it('renders children', () => {
    render(<ThemedBox>盒子内容</ThemedBox>);
    expect(screen.getByText('盒子内容')).toBeInTheDocument();
  });

  it('applies default theme styling', () => {
    render(<ThemedBox>内容</ThemedBox>);
    const box = screen.getByTestId('themed-box');
    expect(box.className).toContain('bg-white');
  });

  it('applies primary theme styling', () => {
    render(<ThemedBox theme="primary">内容</ThemedBox>);
    const box = screen.getByTestId('themed-box');
    expect(box.className).toContain('bg-blue-50');
  });

  it('applies error theme styling', () => {
    render(<ThemedBox theme="error">内容</ThemedBox>);
    const box = screen.getByTestId('themed-box');
    expect(box.className).toContain('bg-red-50');
  });

  it('applies small padding', () => {
    render(<ThemedBox padding="sm">内容</ThemedBox>);
    const box = screen.getByTestId('themed-box');
    expect(box.className).toContain('p-2');
  });

  it('applies rounded class when rounded is true', () => {
    render(<ThemedBox rounded={true}>内容</ThemedBox>);
    const box = screen.getByTestId('themed-box');
    expect(box.className).toContain('rounded-xl');
  });

  it('applies default rounded-lg when rounded is false', () => {
    render(<ThemedBox rounded={false}>内容</ThemedBox>);
    const box = screen.getByTestId('themed-box');
    expect(box.className).toContain('rounded-lg');
    expect(box.className).not.toContain('rounded-xl');
  });

  it('applies custom className', () => {
    render(<ThemedBox className="extra">内容</ThemedBox>);
    expect(screen.getByTestId('themed-box').className).toContain('extra');
  });
});
