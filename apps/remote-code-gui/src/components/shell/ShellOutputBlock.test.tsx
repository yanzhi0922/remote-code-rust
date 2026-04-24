import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ShellOutputBlock } from './ShellOutputBlock';

describe('ShellOutputBlock', () => {
  afterEach(cleanup);

  it('renders command and output', () => {
    render(
      <ShellOutputBlock command="ls -la" output="file1.txt\nfile2.txt" />,
    );
    expect(screen.getByTestId('shell-output-block')).toBeInTheDocument();
    expect(screen.getByText('ls -la')).toBeInTheDocument();
    expect(screen.getByText(/file1\.txt/)).toBeInTheDocument();
  });

  it('shows red border when exitCode is non-zero', () => {
    render(
      <ShellOutputBlock command="fail" output="error" exitCode={1} />,
    );
    const el = screen.getByTestId('shell-output-block');
    expect(el.className).toContain('border-red-500');
  });

  it('shows default border when exitCode is 0', () => {
    render(
      <ShellOutputBlock command="ok" output="done" exitCode={0} />,
    );
    const el = screen.getByTestId('shell-output-block');
    expect(el.className).toContain('border-slate-700');
    expect(el.className).not.toContain('border-red-500');
  });

  it('displays duration when provided', () => {
    render(
      <ShellOutputBlock command="cmd" output="out" duration={2500} />,
    );
    expect(screen.getByText(/2s/)).toBeInTheDocument();
  });

  it('displays exit code when provided', () => {
    render(
      <ShellOutputBlock command="cmd" output="out" exitCode={42} />,
    );
    expect(screen.getByText(/exit: 42/)).toBeInTheDocument();
  });

  it('shows stderr label when stream is stderr', () => {
    render(
      <ShellOutputBlock command="cmd" output="err" stream="stderr" />,
    );
    expect(screen.getByText('stderr')).toBeInTheDocument();
  });

  it('applies custom className', () => {
    render(
      <ShellOutputBlock command="cmd" output="out" className="my-custom" />,
    );
    const el = screen.getByTestId('shell-output-block');
    expect(el.className).toContain('my-custom');
  });

  it('formats long duration correctly', () => {
    render(
      <ShellOutputBlock command="cmd" output="out" duration={125000} />,
    );
    expect(screen.getByText(/2m 5s/)).toBeInTheDocument();
  });
});
