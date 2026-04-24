import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ShellProgress } from './ShellProgress';

describe('ShellProgress', () => {
  afterEach(cleanup);

  it('renders command text', () => {
    render(<ShellProgress command="npm build" elapsedTime={500} />);
    expect(screen.getByTestId('shell-progress')).toHaveTextContent('npm build');
  });

  it('displays elapsed time in ms', () => {
    render(<ShellProgress command="cmd" elapsedTime={200} />);
    expect(screen.getByTestId('shell-progress')).toHaveTextContent('200ms');
  });

  it('displays elapsed time in seconds', () => {
    render(<ShellProgress command="cmd" elapsedTime={3000} />);
    expect(screen.getByTestId('shell-progress')).toHaveTextContent('3s');
  });

  it('displays elapsed time in minutes', () => {
    render(<ShellProgress command="cmd" elapsedTime={125000} />);
    expect(screen.getByTestId('shell-progress')).toHaveTextContent('2m 5s');
  });

  it('applies custom className', () => {
    render(<ShellProgress command="cmd" elapsedTime={100} className="custom" />);
    expect(screen.getByTestId('shell-progress').className).toContain('custom');
  });

  it('has terminal icon', () => {
    render(<ShellProgress command="cmd" elapsedTime={100} />);
    expect(screen.getByTestId('shell-progress')).toBeInTheDocument();
  });
});
