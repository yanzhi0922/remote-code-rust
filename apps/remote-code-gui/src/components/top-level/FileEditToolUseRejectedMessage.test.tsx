import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { FileEditToolUseRejectedMessage } from './FileEditToolUseRejectedMessage';

afterEach(() => {
  cleanup();
});

describe('FileEditToolUseRejectedMessage', () => {
  it('renders rejection message', () => {
    render(<FileEditToolUseRejectedMessage filePath="src/app.tsx" />);
    expect(screen.getByTestId('file-edit-tool-rejected')).toBeInTheDocument();
    expect(screen.getByText('src/app.tsx')).toBeInTheDocument();
  });

  it('shows reason', () => {
    render(<FileEditToolUseRejectedMessage filePath="src/app.tsx" reason="权限不足" />);
    expect(screen.getByText('权限不足')).toBeInTheDocument();
  });
});
