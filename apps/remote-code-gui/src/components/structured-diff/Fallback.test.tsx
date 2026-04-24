import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Fallback } from './Fallback';
import type { StructuredDiffFile } from '../diff/StructuredDiff';

afterEach(() => {
  cleanup();
});

const SAMPLE_DIFFS: StructuredDiffFile[] = [
  { file_path: 'src/app.tsx', hunks: [{ header: '@@ -1,3 +1,4 @@', changes: [] }] },
  { file_path: 'src/utils.ts', hunks: [{ header: '@@ -10,3 +10,3 @@', changes: [] }] },
];

describe('Fallback', () => {
  it('renders fallback diff view', () => {
    render(<Fallback diffs={SAMPLE_DIFFS} />);
    expect(screen.getByTestId('structured-diff-fallback')).toBeInTheDocument();
  });

  it('shows file paths', () => {
    render(<Fallback diffs={SAMPLE_DIFFS} />);
    expect(screen.getByText('src/app.tsx')).toBeInTheDocument();
    expect(screen.getByText('src/utils.ts')).toBeInTheDocument();
  });

  it('shows error message', () => {
    render(<Fallback diffs={[]} error="解析失败" />);
    expect(screen.getByText('解析失败')).toBeInTheDocument();
  });

  it('shows empty diffs', () => {
    render(<Fallback diffs={[]} />);
    expect(screen.getByTestId('structured-diff-fallback')).toBeInTheDocument();
  });
});
