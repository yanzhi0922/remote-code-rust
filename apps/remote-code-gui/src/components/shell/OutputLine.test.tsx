import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { OutputLine } from './OutputLine';

describe('OutputLine', () => {
  afterEach(cleanup);

  it('renders content text', () => {
    render(<OutputLine content="hello world" lineType="stdout" />);
    expect(screen.getByTestId('output-line')).toHaveTextContent('hello world');
  });

  it('applies stderr red styling', () => {
    render(<OutputLine content="error!" lineType="stderr" />);
    const el = screen.getByTestId('output-line');
    const span = el.querySelector('span');
    expect(span?.className).toContain('text-red-400');
  });

  it('applies command green styling', () => {
    render(<OutputLine content="$ ls" lineType="command" />);
    const el = screen.getByTestId('output-line');
    const span = el.querySelector('span');
    expect(span?.className).toContain('text-green-400');
  });

  it('applies info grey styling', () => {
    render(<OutputLine content="info msg" lineType="info" />);
    const el = screen.getByTestId('output-line');
    const span = el.querySelector('span');
    expect(span?.className).toContain('text-slate-500');
  });

  it('shows line number when provided', () => {
    const { container } = render(
      <OutputLine content="line" lineType="stdout" lineNum={42} />,
    );
    expect(container.querySelector('.text-slate-600')).toHaveTextContent('42');
  });

  it('hides line number when omitted', () => {
    const { container } = render(
      <OutputLine content="line" lineType="stdout" />,
    );
    expect(container.querySelector('.text-slate-600')).toBeNull();
  });

  it('applies custom className', () => {
    render(<OutputLine content="x" lineType="stdout" className="my-class" />);
    const el = screen.getByTestId('output-line');
    expect(el.className).toContain('my-class');
  });
});
