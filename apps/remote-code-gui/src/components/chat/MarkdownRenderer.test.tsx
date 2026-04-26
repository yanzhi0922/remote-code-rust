import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import MarkdownRenderer from './MarkdownRenderer';

afterEach(() => { cleanup(); });

describe('MarkdownRenderer', () => {
  it('renders plain text content', () => {
    render(<MarkdownRenderer content="Hello world" />);
    expect(screen.getByText('Hello world')).toBeInTheDocument();
  });

  it('renders bold markdown', () => {
    render(<MarkdownRenderer content="**bold**" />);
    expect(screen.getByText('bold')).toBeInTheDocument();
  });

  it('renders inline code', () => {
    render(<MarkdownRenderer content="`inline`" />);
    expect(screen.getByText('inline')).toBeInTheDocument();
  });

  it('renders links', () => {
    render(<MarkdownRenderer content="[link](https://example.com)" />);
    expect(screen.getByText('link')).toBeInTheDocument();
  });

  it('renders empty content without errors', () => {
    const { container } = render(<MarkdownRenderer content="" />);
    expect(container).toBeInTheDocument();
  });
});
