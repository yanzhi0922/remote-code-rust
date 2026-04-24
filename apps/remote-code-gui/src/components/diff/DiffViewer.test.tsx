import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { DiffViewer } from './DiffViewer';

describe('DiffViewer', () => {
  afterEach(cleanup);

  it('renders file name in header', () => {
    render(
      <DiffViewer oldContent="hello" newContent="world" fileName="test.ts" />,
    );
    expect(screen.getByText('test.ts')).toBeInTheDocument();
  });

  it('shows additions and deletions stats', () => {
    render(
      <DiffViewer oldContent="hello" newContent="world" />,
    );
    expect(screen.getByTestId('diff-stats')).toBeInTheDocument();
  });

  it('renders unified mode by default', () => {
    const oldContent = ['a', 'b', 'c'].join('\n');
    const newContent = ['a', 'x', 'c'].join('\n');
    render(
      <DiffViewer oldContent={oldContent} newContent={newContent} />,
    );
    expect(screen.getByTestId('diff-viewer')).toBeInTheDocument();
    expect(screen.getByTestId('diff-line-delete')).toBeInTheDocument();
    expect(screen.getByTestId('diff-line-add')).toBeInTheDocument();
  });

  it('toggles to side-by-side mode', () => {
    const oldContent = ['a', 'b', 'c'].join('\n');
    const newContent = ['a', 'x', 'c'].join('\n');
    render(
      <DiffViewer oldContent={oldContent} newContent={newContent} />,
    );
    fireEvent.click(screen.getByTestId('diff-mode-toggle'));
    expect(screen.getByTestId('diff-side-by-side')).toBeInTheDocument();
  });

  it('toggles collapse', () => {
    const oldContent = ['a', 'b', 'c', 'd', 'e'].join('\n');
    const newContent = ['a', 'b', 'x', 'd', 'e'].join('\n');
    render(
      <DiffViewer oldContent={oldContent} newContent={newContent} />,
    );
    fireEvent.click(screen.getByTestId('diff-collapse-toggle'));
    expect(screen.getByTestId('diff-collapse-toggle')).toBeInTheDocument();
  });

  it('renders context lines for unchanged content', () => {
    const content = ['same', 'line'].join('\n');
    render(
      <DiffViewer oldContent={content} newContent={content} />,
    );
    expect(screen.getAllByTestId('diff-line-context').length).toBeGreaterThan(0);
  });

  it('renders hunk header at change boundaries', () => {
    const oldContent = ['a', 'b', 'c'].join('\n');
    const newContent = ['a', 'x', 'c'].join('\n');
    render(
      <DiffViewer oldContent={oldContent} newContent={newContent} />,
    );
    expect(screen.getByTestId('diff-line-hunk_header')).toBeInTheDocument();
  });
});
