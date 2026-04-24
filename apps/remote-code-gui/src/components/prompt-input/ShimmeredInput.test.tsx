import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ShimmeredInput } from './ShimmeredInput';

describe('ShimmeredInput', () => {
  afterEach(cleanup);

  it('渲染并显示 data-testid', () => {
    render(<ShimmeredInput />);
    expect(screen.getByTestId('shimmered-input')).toBeInTheDocument();
  });

  it('包含 shimmer 动画元素', () => {
    render(<ShimmeredInput />);
    const el = screen.getByTestId('shimmered-input');
    const shimmer = el.querySelector('[class*="animate"]');
    expect(shimmer).toBeInTheDocument();
  });

  it('应用默认样式', () => {
    render(<ShimmeredInput />);
    const el = screen.getByTestId('shimmered-input');
    expect(el.className).toContain('rounded-lg');
    expect(el.className).toContain('bg-slate-100');
  });

  it('支持自定义 className', () => {
    render(<ShimmeredInput className="w-96" />);
    const el = screen.getByTestId('shimmered-input');
    expect(el.className).toContain('w-96');
  });

  it('有固定高度', () => {
    render(<ShimmeredInput />);
    const el = screen.getByTestId('shimmered-input');
    expect(el.className).toContain('h-10');
  });
});
