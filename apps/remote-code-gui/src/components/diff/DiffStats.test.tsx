import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { DiffStats } from './DiffStats';

describe('DiffStats', () => {
  afterEach(cleanup);

  it('renders additions in green', () => {
    render(<DiffStats additions={10} deletions={3} filesChanged={2} />);
    expect(screen.getByText('+10')).toHaveClass('text-green-600');
  });

  it('renders deletions in red', () => {
    render(<DiffStats additions={10} deletions={3} filesChanged={2} />);
    expect(screen.getByText('-3')).toHaveClass('text-red-600');
  });

  it('renders file count', () => {
    render(<DiffStats additions={10} deletions={3} filesChanged={2} />);
    expect(screen.getByText('2 个文件')).toBeInTheDocument();
  });

  it('renders progress bar', () => {
    render(<DiffStats additions={10} deletions={3} filesChanged={2} />);
    expect(screen.getByTestId('diff-stats-bar')).toBeInTheDocument();
  });

  it('handles zero additions and deletions', () => {
    render(<DiffStats additions={0} deletions={0} filesChanged={0} />);
    expect(screen.getByText('+0')).toBeInTheDocument();
    expect(screen.getByText('-0')).toBeInTheDocument();
    expect(screen.getByText('0 个文件')).toBeInTheDocument();
  });

  it('handles all additions', () => {
    render(<DiffStats additions={5} deletions={0} filesChanged={1} />);
    expect(screen.getByText('+5')).toBeInTheDocument();
    expect(screen.getByText('-0')).toBeInTheDocument();
  });
});
