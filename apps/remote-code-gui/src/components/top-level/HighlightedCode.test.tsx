import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { HighlightedCode } from './HighlightedCode';

describe('HighlightedCode', () => {
  afterEach(() => { cleanup(); });

  it('renders code content', () => {
    const code = ['line1', 'line2'].join('\n');
    const { getByTestId } = render(
      <HighlightedCode code={code} filePath="test.ts" />,
    );
    expect(getByTestId('highlighted-code')).toBeInTheDocument();
    // Verify code text is present somewhere
    expect(getByTestId('highlighted-code').textContent).toContain('line1');
    expect(getByTestId('highlighted-code').textContent).toContain('line2');
  });

  it('shows file path in header', () => {
    const { getByText } = render(
      <HighlightedCode code="code" filePath="app.tsx" />,
    );
    expect(getByText('app.tsx')).toBeInTheDocument();
  });

  it('shows language label when provided', () => {
    const { getByText } = render(
      <HighlightedCode code="code" filePath="f.ts" language="typescript" />,
    );
    expect(getByText('typescript')).toBeInTheDocument();
  });

  it('applies dim class when dim is true', () => {
    const { getByTestId } = render(
      <HighlightedCode code="x" filePath="f.ts" dim />,
    );
    expect(getByTestId('highlighted-code').className).toContain('opacity-60');
  });
});
