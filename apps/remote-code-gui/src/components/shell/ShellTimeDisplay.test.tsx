import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ShellTimeDisplay } from './ShellTimeDisplay';

describe('ShellTimeDisplay', () => {
  afterEach(cleanup);

  it('shows running when endTime is not provided', () => {
    render(<ShellTimeDisplay startTime={1000} />);
    expect(screen.getByTestId('shell-time-display')).toHaveTextContent('running...');
  });

  it('shows elapsed time in ms for short durations', () => {
    render(<ShellTimeDisplay startTime={1000} endTime={1050} />);
    expect(screen.getByTestId('shell-time-display')).toHaveTextContent('50ms');
  });

  it('shows elapsed time in seconds', () => {
    render(<ShellTimeDisplay startTime={1000} endTime={4500} />);
    expect(screen.getByTestId('shell-time-display')).toHaveTextContent('3s');
  });

  it('shows elapsed time in minutes and seconds', () => {
    render(<ShellTimeDisplay startTime={0} endTime={125000} />);
    expect(screen.getByTestId('shell-time-display')).toHaveTextContent('2m 5s');
  });

  it('shows elapsed time in hours and minutes', () => {
    render(<ShellTimeDisplay startTime={0} endTime={3700000} />);
    expect(screen.getByTestId('shell-time-display')).toHaveTextContent('1h 1m');
  });

  it('applies custom className', () => {
    render(
      <ShellTimeDisplay startTime={1000} endTime={2000} className="my-cls" />,
    );
    const el = screen.getByTestId('shell-time-display');
    expect(el.className).toContain('my-cls');
  });

  it('has spinner icon when running', () => {
    const { container } = render(<ShellTimeDisplay startTime={1000} />);
    const spinner = container.querySelector('.animate-spin');
    expect(spinner).toBeInTheDocument();
  });

  it('has clock icon when completed', () => {
    render(<ShellTimeDisplay startTime={1000} endTime={2000} />);
    expect(screen.getByTestId('shell-time-display')).toHaveTextContent('1s');
  });
});
