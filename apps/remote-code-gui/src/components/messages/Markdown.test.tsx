import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Markdown } from './Markdown';

describe('Markdown', () => {
  afterEach(cleanup);

  it('渲染纯文本', () => {
    render(<Markdown>Hello world</Markdown>);
    expect(screen.getByTestId('markdown-content')).toBeInTheDocument();
    expect(screen.getByText('Hello world')).toBeInTheDocument();
  });

  it('渲染粗体文本', () => {
    render(<Markdown>{'This is **bold** text'}</Markdown>);
    const bold = screen.getByText('bold');
    expect(bold.tagName).toBe('STRONG');
  });

  it('渲染斜体文本', () => {
    render(<Markdown>{'This is *italic* text'}</Markdown>);
    const italic = screen.getByText('italic');
    expect(italic.tagName).toBe('EM');
  });

  it('渲染行内代码', () => {
    render(<Markdown>{'Use `console.log` here'}</Markdown>);
    const code = screen.getByText('console.log');
    expect(code.tagName).toBe('CODE');
  });

  it('渲染代码块', () => {
    render(<Markdown>{'```\nconst x = 1;\n```'}</Markdown>);
    expect(screen.getByText('const x = 1;')).toBeInTheDocument();
  });

  it('渲染链接', () => {
    render(<Markdown>{'[Click here](https://example.com)'}</Markdown>);
    const link = screen.getByText('Click here');
    expect(link.tagName).toBe('A');
    expect(link).toHaveAttribute('href', 'https://example.com');
  });

  it('dimColor 模式使用暗淡颜色', () => {
    const { container } = render(<Markdown dimColor>dim text</Markdown>);
    expect(container.firstChild).toHaveClass('text-slate-500');
  });

  it('应用自定义 className', () => {
    const { container } = render(
      <Markdown className="md-custom">text</Markdown>,
    );
    expect(container.firstChild).toHaveClass('md-custom');
  });
});
