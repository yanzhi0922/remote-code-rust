import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { Markdown } from './Markdown';

describe('Markdown', () => {
  afterEach(() => { cleanup(); });

  it('renders plain text as paragraphs', () => {
    const { getByTestId, getByText } = render(<Markdown>Hello world</Markdown>);
    expect(getByTestId('markdown')).toBeInTheDocument();
    expect(getByText('Hello world')).toBeInTheDocument();
  });

  it('renders code blocks', () => {
    const { getByTestId } = render(<Markdown>{'```\ncode here\n```'}</Markdown>);
    expect(getByTestId('markdown-code-block')).toBeInTheDocument();
  });

  it('renders headers', () => {
    const { getByText } = render(<Markdown>{'# Title\n## Subtitle\n### Section'}</Markdown>);
    expect(getByText('Title')).toBeInTheDocument();
    expect(getByText('Subtitle')).toBeInTheDocument();
    expect(getByText('Section')).toBeInTheDocument();
  });

  it('renders list items', () => {
    const { getByText } = render(<Markdown>{'- item1\n* item2'}</Markdown>);
    expect(getByText('item1')).toBeInTheDocument();
    expect(getByText('item2')).toBeInTheDocument();
  });

  it('applies dimColor class', () => {
    const { getByTestId } = render(<Markdown dimColor>text</Markdown>);
    expect(getByTestId('markdown').className).toContain('opacity-60');
  });
});
