import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { StatusLine } from './StatusLine';

afterEach(() => {
  cleanup();
});

describe('StatusLine', () => {
  it('renders idle status', () => {
    render(<StatusLine status="idle" />);
    expect(screen.getByTestId('status-line')).toBeInTheDocument();
    expect(screen.getByText('空闲')).toBeInTheDocument();
  });

  it('renders running status', () => {
    render(<StatusLine status="running" />);
    expect(screen.getByText('运行中')).toBeInTheDocument();
  });

  it('renders custom message', () => {
    render(<StatusLine status="running" message="正在处理..." />);
    expect(screen.getByText('正在处理...')).toBeInTheDocument();
  });
});
