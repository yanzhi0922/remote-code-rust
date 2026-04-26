import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { CodeBlock } from './CodeBlock';

afterEach(() => { cleanup(); });

describe('CodeBlock', () => {
  it('renders code content', () => {
    render(<CodeBlock code="console.log('hello')" />);
    expect(screen.getByText(/console\.log/)).toBeInTheDocument();
  });

  it('renders without language', () => {
    render(<CodeBlock code="plain text" />);
    expect(screen.getByText('plain text')).toBeInTheDocument();
  });

  it('renders language label when provided', () => {
    render(<CodeBlock code="x = 1" language="python" />);
    expect(screen.getByText('python')).toBeInTheDocument();
  });

  it('has copy button', () => {
    render(<CodeBlock code="test" />);
    expect(screen.getByLabelText('Copy code')).toBeInTheDocument();
  });
});
