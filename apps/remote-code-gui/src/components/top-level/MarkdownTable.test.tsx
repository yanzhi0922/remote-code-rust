import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { MarkdownTable } from './MarkdownTable';

afterEach(() => {
  cleanup();
});

describe('MarkdownTable', () => {
  it('renders table with headers and rows', () => {
    render(<MarkdownTable headers={['名称', '值']} rows={[['A', '1'], ['B', '2']]} />);
    expect(screen.getByTestId('markdown-table')).toBeInTheDocument();
    expect(screen.getByTestId('markdown-table-header-0')).toHaveTextContent('名称');
    expect(screen.getByTestId('markdown-table-row-0')).toBeInTheDocument();
  });

  it('renders empty table', () => {
    render(<MarkdownTable headers={[]} rows={[]} />);
    expect(screen.getByTestId('markdown-table')).toBeInTheDocument();
  });
});
