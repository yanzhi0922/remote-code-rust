import { afterEach, describe, it, expect } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { FileEditToolDiff } from './FileEditToolDiff';

afterEach(() => {
  cleanup();
});

const diffLines = [
  { type: 'context' as const, content: 'line 1', lineNumber: 1 },
  { type: 'remove' as const, content: 'old line', lineNumber: 2 },
  { type: 'add' as const, content: 'new line', lineNumber: 2 },
  { type: 'context' as const, content: 'line 3', lineNumber: 3 },
];

describe('FileEditToolDiff', () => {
  it('renders diff container', () => {
    render(<FileEditToolDiff filePath="test.ts" diffLines={diffLines} />);
    expect(screen.getByTestId('file-edit-diff')).toBeInTheDocument();
  });

  it('shows file path', () => {
    render(<FileEditToolDiff filePath="src/app.tsx" diffLines={diffLines} />);
    expect(screen.getByText('src/app.tsx')).toBeInTheDocument();
  });

  it('renders diff lines with correct styling', () => {
    render(<FileEditToolDiff filePath="test.ts" diffLines={diffLines} />);
    const removeLine = screen.getByTestId('diff-line-1');
    expect(removeLine.classList.contains('bg-red-50')).toBe(true);
    const addLine = screen.getByTestId('diff-line-2');
    expect(addLine.classList.contains('bg-green-50')).toBe(true);
  });

  it('shows expand button for long diffs', () => {
    const longLines = Array.from({ length: 25 }, (_, i) => ({
      type: 'context' as const,
      content: `line ${i}`,
      lineNumber: i + 1,
    }));
    render(<FileEditToolDiff filePath="test.ts" diffLines={longLines} />);
    expect(screen.getByTestId('diff-expand')).toBeInTheDocument();
  });

  it('expands all lines on click', () => {
    const longLines = Array.from({ length: 25 }, (_, i) => ({
      type: 'context' as const,
      content: `line ${i}`,
      lineNumber: i + 1,
    }));
    render(<FileEditToolDiff filePath="test.ts" diffLines={longLines} />);
    fireEvent.click(screen.getByTestId('diff-expand'));
    expect(screen.queryByTestId('diff-expand')).not.toBeInTheDocument();
  });

  it('renders line numbers', () => {
    render(<FileEditToolDiff filePath="test.ts" diffLines={diffLines} />);
    const container = screen.getByTestId('file-edit-diff');
    // Line numbers are rendered inside diff lines
    expect(container.textContent).toContain('1');
    expect(container.textContent).toContain('2');
    expect(container.textContent).toContain('3');
  });
});
