import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { FileEditToolUpdatedMessage } from './FileEditToolUpdatedMessage';

afterEach(() => {
  cleanup();
});

describe('FileEditToolUpdatedMessage', () => {
  it('renders file path', () => {
    render(<FileEditToolUpdatedMessage filePath="src/app.tsx" />);
    expect(screen.getByTestId('file-edit-tool-updated')).toBeInTheDocument();
    expect(screen.getByText('src/app.tsx')).toBeInTheDocument();
  });

  it('shows description', () => {
    render(<FileEditToolUpdatedMessage filePath="src/app.tsx" description="添加了新函数" />);
    expect(screen.getByText('添加了新函数')).toBeInTheDocument();
  });
});
