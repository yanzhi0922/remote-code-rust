import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { FilePathLink } from './FilePathLink';

describe('FilePathLink', () => {
  afterEach(() => { cleanup(); });

  it('renders file path as text', () => {
    const { getByTestId, getByText } = render(<FilePathLink filePath="/src/index.ts" />);
    expect(getByTestId('file-path-link')).toBeInTheDocument();
    expect(getByText('/src/index.ts')).toBeInTheDocument();
  });

  it('renders children as display text', () => {
    const { getByText } = render(
      <FilePathLink filePath="/src/index.ts">index.ts</FilePathLink>,
    );
    expect(getByText('index.ts')).toBeInTheDocument();
  });

  it('calls onClick with file path when clicked', () => {
    const onClick = vi.fn();
    const { getByTestId } = render(
      <FilePathLink filePath="/src/app.tsx" onClick={onClick} />,
    );
    fireEvent.click(getByTestId('file-path-link'));
    expect(onClick).toHaveBeenCalledWith('/src/app.tsx');
  });

  it('sets title attribute to filePath', () => {
    const { getByTestId } = render(<FilePathLink filePath="/src/main.rs" />);
    expect(getByTestId('file-path-link').getAttribute('title')).toBe('/src/main.rs');
  });
});
