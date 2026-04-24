import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { NotebookEditToolDiff } from './NotebookEditToolDiff';

describe('NotebookEditToolDiff', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<NotebookEditToolDiff cellIndex={0} oldSource="old" newSource="new" />);
    expect(screen.getByTestId('notebook-edit-tool-diff')).toBeInTheDocument();
  });

  it('shows cell index', () => {
    render(<NotebookEditToolDiff cellIndex={3} oldSource="a" newSource="b" />);
    expect(screen.getByText('Cell #3')).toBeInTheDocument();
  });

  it('shows old and new source', () => {
    render(<NotebookEditToolDiff cellIndex={0} oldSource="print('old')" newSource="print('new')" />);
    expect(screen.getByText("print('old')")).toBeInTheDocument();
    expect(screen.getByText("print('new')")).toBeInTheDocument();
  });
});
