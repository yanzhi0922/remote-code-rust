import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ShellProgressMessage } from './ShellProgressMessage';

describe('ShellProgressMessage', () => {
  afterEach(cleanup);

  it('renders command text', () => {
    render(<ShellProgressMessage command="npm install" />);
    expect(screen.getByTestId('shell-progress-message')).toHaveTextContent('npm install');
  });

  it('shows spinner when no progress provided', () => {
    const { container } = render(<ShellProgressMessage command="cmd" />);
    const spinner = container.querySelector('.animate-spin');
    expect(spinner).toBeInTheDocument();
  });

  it('shows progress bar when progress and total provided', () => {
    const { container } = render(
      <ShellProgressMessage command="cmd" progress={50} total={100} />,
    );
    const bar = container.querySelector('[style*="width: 50%"]');
    expect(bar).toBeInTheDocument();
  });

  it('shows percentage text when progress provided', () => {
    render(<ShellProgressMessage command="cmd" progress={75} total={100} />);
    expect(screen.getByText('75%')).toBeInTheDocument();
  });

  it('clamps progress to 100%', () => {
    render(<ShellProgressMessage command="cmd" progress={200} total={100} />);
    expect(screen.getByText('100%')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(<ShellProgressMessage command="cmd" className="custom-cls" />);
    const el = screen.getByTestId('shell-progress-message');
    expect(el.className).toContain('custom-cls');
  });

  it('shows spinner when total is 0', () => {
    const { container } = render(
      <ShellProgressMessage command="cmd" progress={0} total={0} />,
    );
    const spinner = container.querySelector('.animate-spin');
    expect(spinner).toBeInTheDocument();
  });
});
