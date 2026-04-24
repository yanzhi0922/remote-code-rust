import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Spinner } from './Spinner';
import { ProgressBar } from './ProgressBar';
import { CodeBlock } from './CodeBlock';

afterEach(() => {
  cleanup();
});

// ─── Spinner ────────────────────────────────────────────────────────
describe('Spinner', () => {
  it('renders with default medium size', () => {
    render(<Spinner />);
    const spinner = screen.getByTestId('spinner');
    expect(spinner).toBeInTheDocument();
    expect(spinner.getAttribute('class')).toContain('h-6');
    expect(spinner.getAttribute('class')).toContain('w-6');
  });

  it('renders small size', () => {
    render(<Spinner size="sm" />);
    const spinner = screen.getByTestId('spinner');
    expect(spinner.getAttribute('class')).toContain('h-4');
    expect(spinner.getAttribute('class')).toContain('w-4');
  });

  it('renders large size', () => {
    render(<Spinner size="lg" />);
    const spinner = screen.getByTestId('spinner');
    expect(spinner.getAttribute('class')).toContain('h-8');
    expect(spinner.getAttribute('class')).toContain('w-8');
  });

  it('applies custom color via style', () => {
    render(<Spinner color="#ff0000" />);
    const spinner = screen.getByTestId('spinner');
    expect(spinner.style.color).toBe('rgb(255, 0, 0)');
  });

  it('has animate-spin class', () => {
    render(<Spinner />);
    const spinner = screen.getByTestId('spinner');
    expect(spinner.getAttribute('class')).toContain('animate-spin');
  });

  it('does not set color style when color prop is omitted', () => {
    render(<Spinner />);
    const spinner = screen.getByTestId('spinner');
    expect(spinner.style.color).toBe('');
  });
});

// ─── ProgressBar ────────────────────────────────────────────────────
describe('ProgressBar', () => {
  it('renders with correct width percentage', () => {
    render(<ProgressBar value={50} max={100} />);
    const bar = screen.getByTestId('progress-bar');
    expect(bar).toBeInTheDocument();
    const inner = bar.querySelector('[role="progressbar"]') as HTMLElement;
    expect(inner.style.width).toBe('50%');
  });

  it('clamps value over 100%', () => {
    render(<ProgressBar value={150} max={100} />);
    const inner = screen.getByTestId('progress-bar').querySelector('[role="progressbar"]') as HTMLElement;
    expect(inner.style.width).toBe('100%');
  });

  it('shows label when showLabel is true', () => {
    render(<ProgressBar value={75} max={100} showLabel />);
    expect(screen.getByTestId('progress-label')).toHaveTextContent('75%');
  });

  it('hides label by default', () => {
    render(<ProgressBar value={50} max={100} />);
    expect(screen.queryByTestId('progress-label')).not.toBeInTheDocument();
  });

  it('applies small size class', () => {
    render(<ProgressBar value={50} max={100} size="sm" />);
    const inner = screen.getByTestId('progress-bar').querySelector('[role="progressbar"]') as HTMLElement;
    expect(inner.className).toContain('h-1.5');
  });

  it('handles zero max gracefully', () => {
    render(<ProgressBar value={10} max={0} />);
    const inner = screen.getByTestId('progress-bar').querySelector('[role="progressbar"]') as HTMLElement;
    expect(inner.style.width).toBe('0%');
  });
});

// ─── CodeBlock ──────────────────────────────────────────────────────
describe('CodeBlock', () => {
  it('renders code content', () => {
    render(<CodeBlock code="console.log('hello')" />);
    expect(screen.getByTestId('code-block')).toHaveTextContent("console.log('hello')");
  });

  it('shows language label when provided', () => {
    render(<CodeBlock code="let x = 1" language="typescript" />);
    expect(screen.getByTestId('code-language')).toHaveTextContent('typescript');
  });

  it('hides language label when not provided', () => {
    render(<CodeBlock code="let x = 1" />);
    expect(screen.queryByTestId('code-language')).not.toBeInTheDocument();
  });

  it('shows line numbers when enabled', () => {
    render(<CodeBlock code={'line1\nline2\nline3'} showLineNumbers />);
    expect(screen.getByText('1')).toBeInTheDocument();
    expect(screen.getByText('2')).toBeInTheDocument();
    expect(screen.getByText('3')).toBeInTheDocument();
  });

  it('hides line numbers by default', () => {
    render(<CodeBlock code={'line1\nline2'} />);
    expect(screen.queryByText('1')).not.toBeInTheDocument();
  });

  it('copies code to clipboard on copy button click', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    render(<CodeBlock code="test-code" />);
    fireEvent.click(screen.getByTestId('copy-button'));
    expect(writeText).toHaveBeenCalledWith('test-code');
  });

  it('applies maxHeight style when provided', () => {
    render(<CodeBlock code="code" maxHeight={200} />);
    const container = screen.getByTestId('code-block').querySelector('.overflow-auto') as HTMLElement;
    expect(container.style.maxHeight).toBe('200px');
  });
});
