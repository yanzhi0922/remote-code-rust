import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { StructuredDiffList } from './StructuredDiffList';

afterEach(() => {
  cleanup();
});

const DIFFS = [
  {
    file_path: 'src/app.tsx',
    hunks: [{ header: '@@ -1,3 +1,4 @@', changes: [{ type: 'add' as const, content: 'new' }] }],
  },
  {
    file_path: 'src/util.ts',
    hunks: [{ header: '@@ -1,3 +1,3 @@', changes: [{ type: 'delete' as const, content: 'old' }] }],
  },
];

describe('StructuredDiffList', () => {
  it('renders file list', () => {
    render(<StructuredDiffList diffs={DIFFS} />);
    expect(screen.getByTestId('structured-diff-list')).toBeInTheDocument();
  });

  it('shows empty state', () => {
    render(<StructuredDiffList diffs={[]} />);
    expect(screen.getByTestId('structured-diff-list-empty')).toHaveTextContent('无变更文件');
  });

  it('calls onSelect', () => {
    const onSelect = vi.fn();
    render(<StructuredDiffList diffs={DIFFS} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId('diff-list-item-src-app.tsx'));
    expect(onSelect).toHaveBeenCalledWith('src/app.tsx');
  });
});
