import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { FileWriteToolDiff } from './FileWriteToolDiff';

describe('FileWriteToolDiff', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<FileWriteToolDiff oldContent="a" newContent="b" />);
    expect(screen.getByTestId('file-write-tool-diff')).toBeInTheDocument();
  });

  it('shows old and new content', () => {
    render(<FileWriteToolDiff oldContent="old line" newContent="new line" />);
    expect(screen.getByText('old line')).toBeInTheDocument();
    expect(screen.getByText('new line')).toBeInTheDocument();
  });

  it('shows file path header', () => {
    render(<FileWriteToolDiff oldContent="a" newContent="b" filePath="/src/app.ts" />);
    expect(screen.getByText('/src/app.ts')).toBeInTheDocument();
  });
});
