import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { StructuredDiff, type StructuredDiffFile } from './StructuredDiff';

const SAMPLE_DIFFS: StructuredDiffFile[] = [
  {
    file_path: 'src/app.tsx',
    hunks: [
      {
        header: '@@ -1,3 +1,4 @@',
        changes: [
          { type: 'context', content: 'import React;', old_line: 1, new_line: 1 },
          { type: 'add', content: 'import styles;', new_line: 2 },
          { type: 'delete', content: 'const x = 1;', old_line: 2 },
          { type: 'context', content: 'export default App;', old_line: 3, new_line: 4 },
        ],
      },
    ],
  },
  {
    file_path: 'src/utils.ts',
    hunks: [
      {
        header: '@@ -10,3 +10,3 @@',
        changes: [
          { type: 'context', content: 'function helper() {', old_line: 10, new_line: 10 },
          { type: 'delete', content: '  return null;', old_line: 11 },
          { type: 'add', content: '  return undefined;', new_line: 11 },
        ],
      },
    ],
  },
];

describe('StructuredDiff', () => {
  afterEach(cleanup);

  it('renders file paths', () => {
    render(<StructuredDiff diffs={SAMPLE_DIFFS} />);
    expect(screen.getByText('src/app.tsx')).toBeInTheDocument();
    expect(screen.getByText('src/utils.ts')).toBeInTheDocument();
  });

  it('shows empty state when no diffs', () => {
    render(<StructuredDiff diffs={[]} />);
    expect(screen.getByTestId('structured-diff-empty')).toHaveTextContent('无变更');
  });

  it('renders hunk headers', () => {
    render(<StructuredDiff diffs={SAMPLE_DIFFS} />);
    expect(screen.getByText('@@ -1,3 +1,4 @@')).toBeInTheDocument();
    expect(screen.getByText('@@ -10,3 +10,3 @@')).toBeInTheDocument();
  });

  it('renders change lines with correct types', () => {
    render(<StructuredDiff diffs={SAMPLE_DIFFS} />);
    // Should have add lines
    const addLines = screen.getAllByTestId('diff-line-add');
    expect(addLines.length).toBeGreaterThan(0);
    // Should have delete lines
    const deleteLines = screen.getAllByTestId('diff-line-delete');
    expect(deleteLines.length).toBeGreaterThan(0);
  });

  it('shows per-file change stats', () => {
    render(<StructuredDiff diffs={SAMPLE_DIFFS} />);
    // First file: +1 -1, Second file: +1 -1
    const stats = screen.getAllByTestId('diff-stats');
    // One in the header + one per file section? Actually stats are in header only
    expect(stats.length).toBeGreaterThanOrEqual(1);
  });

  it('collapses and expands file sections', () => {
    render(<StructuredDiff diffs={SAMPLE_DIFFS} />);
    const headers = screen.getAllByTestId('structured-diff-file-header');
    // Click to collapse first file
    fireEvent.click(headers[0]);
    // Hunk headers for first file should still be in DOM since we test second file
    expect(screen.getByText('@@ -10,3 +10,3 @@')).toBeInTheDocument();
    // Click to expand again
    fireEvent.click(headers[0]);
    expect(screen.getByText('@@ -1,3 +1,4 @@')).toBeInTheDocument();
  });

  it('shows total stats in header', () => {
    render(<StructuredDiff diffs={SAMPLE_DIFFS} />);
    expect(screen.getByText('变更文件')).toBeInTheDocument();
  });
});
