import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PreviewBox } from './PreviewBox';

describe('PreviewBox', () => {
  afterEach(cleanup);

  it('renders with data-testid', () => {
    render(<PreviewBox content="hello" />);
    expect(screen.getByTestId('preview-box')).toBeInTheDocument();
  });

  it('shows content', () => {
    render(<PreviewBox content="console.log('hi')" />);
    expect(screen.getByText("console.log('hi')")).toBeInTheDocument();
  });

  it('shows language', () => {
    render(<PreviewBox content="x" language="typescript" />);
    expect(screen.getByText('typescript')).toBeInTheDocument();
  });
});
