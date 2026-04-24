import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { CompactSummary } from './CompactSummary';

afterEach(() => {
  cleanup();
});

describe('CompactSummary', () => {
  it('renders compact summary container', () => {
    render(<CompactSummary />);
    expect(screen.getByTestId('compact-summary')).toBeInTheDocument();
  });

  it('shows "Compact summary" heading without metadata', () => {
    render(<CompactSummary />);
    expect(screen.getByText('Compact summary')).toBeInTheDocument();
  });

  it('shows "Summarized conversation" with metadata', () => {
    render(
      <CompactSummary
        metadata={{ messagesSummarized: 10, direction: 'up_to' }}
      />,
    );
    expect(screen.getByText('Summarized conversation')).toBeInTheDocument();
  });

  it('shows messages count with metadata', () => {
    render(
      <CompactSummary
        metadata={{ messagesSummarized: 10, direction: 'up_to' }}
      />,
    );
    expect(screen.getByText(/Summarized 10 messages/)).toBeInTheDocument();
  });

  it('shows user context when provided', () => {
    render(
      <CompactSummary
        metadata={{ messagesSummarized: 5, direction: 'from_here', userContext: 'fix bugs' }}
      />,
    );
    expect(screen.getByText(/fix bugs/)).toBeInTheDocument();
  });

  it('shows expand button when onExpand is provided', () => {
    render(<CompactSummary onExpand={vi.fn()} />);
    expect(screen.getByTestId('compact-expand')).toBeInTheDocument();
  });

  it('calls onExpand when expand button is clicked', () => {
    const onExpand = vi.fn();
    render(<CompactSummary onExpand={onExpand} />);
    fireEvent.click(screen.getByTestId('compact-expand'));
    expect(onExpand).toHaveBeenCalled();
  });

  it('does not show expand in transcript mode without metadata', () => {
    render(<CompactSummary isTranscriptMode onExpand={vi.fn()} />);
    expect(screen.queryByTestId('compact-expand')).not.toBeInTheDocument();
  });
});
