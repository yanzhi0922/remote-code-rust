import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { DiffLine } from './DiffLine';

describe('DiffLine', () => {
  afterEach(cleanup);

  it('renders context line with content', () => {
    render(<DiffLine type="context" content="hello world" oldLine={1} newLine={1} />);
    expect(screen.getByTestId('diff-line-context')).toHaveTextContent('hello world');
  });

  it('renders add line with green styling', () => {
    render(<DiffLine type="add" content="new line" newLine={2} />);
    const el = screen.getByTestId('diff-line-add');
    expect(el).toHaveTextContent('new line');
    expect(el.className).toContain('bg-green-50');
    // text-green-800 is on inner spans
    const contentSpan = el.querySelector('.whitespace-pre');
    expect(contentSpan?.className).toContain('text-green-800');
  });

  it('renders delete line with red styling', () => {
    render(<DiffLine type="delete" content="old line" oldLine={1} />);
    const el = screen.getByTestId('diff-line-delete');
    expect(el).toHaveTextContent('old line');
    expect(el.className).toContain('bg-red-50');
    // text-red-800 is on inner spans
    const contentSpan = el.querySelector('.whitespace-pre');
    expect(contentSpan?.className).toContain('text-red-800');
  });

  it('renders hunk_header with grey styling', () => {
    render(<DiffLine type="hunk_header" content="@@ -1,3 +1,4 @@" />);
    const el = screen.getByTestId('diff-line-hunk_header');
    expect(el).toHaveTextContent('@@ -1,3 +1,4 @@');
    expect(el.className).toContain('bg-slate-100');
  });

  it('shows line numbers for old and new', () => {
    const { container } = render(
      <DiffLine type="context" content="line" oldLine={5} newLine={6} />,
    );
    const spans = container.querySelectorAll('span');
    expect(spans[0]).toHaveTextContent('5');
    expect(spans[1]).toHaveTextContent('6');
  });

  it('renders without line numbers when omitted', () => {
    const { container } = render(<DiffLine type="hunk_header" content="@@@@" />);
    const spans = container.querySelectorAll('span');
    expect(spans[0]).toHaveTextContent('');
    expect(spans[1]).toHaveTextContent('');
  });
});
