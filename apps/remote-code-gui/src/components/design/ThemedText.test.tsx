import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ThemedText } from './ThemedText';

describe('ThemedText', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<ThemedText>文本</ThemedText>);
    expect(screen.getByTestId('themed-text')).toBeInTheDocument();
  });

  it('renders children text', () => {
    render(<ThemedText>你好世界</ThemedText>);
    expect(screen.getByText('你好世界')).toBeInTheDocument();
  });

  it('applies default theme styling', () => {
    render(<ThemedText>文本</ThemedText>);
    const text = screen.getByTestId('themed-text');
    expect(text.className).toContain('text-slate-800');
  });

  it('applies primary theme styling', () => {
    render(<ThemedText theme="primary">文本</ThemedText>);
    const text = screen.getByTestId('themed-text');
    expect(text.className).toContain('text-blue-600');
  });

  it('applies muted theme styling', () => {
    render(<ThemedText theme="muted">文本</ThemedText>);
    const text = screen.getByTestId('themed-text');
    expect(text.className).toContain('text-slate-400');
  });

  it('applies small size', () => {
    render(<ThemedText size="sm">文本</ThemedText>);
    const text = screen.getByTestId('themed-text');
    expect(text.className).toContain('text-sm');
  });

  it('applies bold styling', () => {
    render(<ThemedText bold={true}>文本</ThemedText>);
    const text = screen.getByTestId('themed-text');
    expect(text.className).toContain('font-bold');
  });

  it('does not apply bold when bold is false', () => {
    render(<ThemedText bold={false}>文本</ThemedText>);
    const text = screen.getByTestId('themed-text');
    expect(text.className).not.toContain('font-bold');
  });

  it('applies custom className', () => {
    render(<ThemedText className="extra">文本</ThemedText>);
    expect(screen.getByTestId('themed-text').className).toContain('extra');
  });
});
