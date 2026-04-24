import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { NotebookEditToolUseRejectedMessage } from './NotebookEditToolUseRejectedMessage';

afterEach(() => {
  cleanup();
});

describe('NotebookEditToolUseRejectedMessage', () => {
  it('renders rejection message', () => {
    render(<NotebookEditToolUseRejectedMessage notebookPath="notebook.ipynb" />);
    expect(screen.getByTestId('notebook-edit-tool-rejected')).toBeInTheDocument();
    expect(screen.getByText(/notebook.ipynb/)).toBeInTheDocument();
  });

  it('shows cell index', () => {
    render(<NotebookEditToolUseRejectedMessage notebookPath="nb.ipynb" cellIndex={3} />);
    expect(screen.getByText(/单元格 3/)).toBeInTheDocument();
  });

  it('shows reason', () => {
    render(<NotebookEditToolUseRejectedMessage notebookPath="nb.ipynb" reason="只读" />);
    expect(screen.getByText('只读')).toBeInTheDocument();
  });
});
