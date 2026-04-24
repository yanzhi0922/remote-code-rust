import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { ApproveApiKeyDialog } from './ApproveApiKeyDialog';

afterEach(() => {
  cleanup();
});

describe('ApproveApiKeyDialog', () => {
  it('renders with data-testid', () => {
    render(<ApproveApiKeyDialog customApiKeyTruncated="abc123" onDone={vi.fn()} />);
    expect(screen.getByTestId('approve-api-key-dialog')).toBeInTheDocument();
  });

  it('shows the truncated API key', () => {
    render(<ApproveApiKeyDialog customApiKeyTruncated="abc123" onDone={vi.fn()} />);
    expect(screen.getByText(/abc123/)).toBeInTheDocument();
  });

  it('shows the title', () => {
    render(<ApproveApiKeyDialog customApiKeyTruncated="abc123" onDone={vi.fn()} />);
    expect(screen.getByText('Approve API Key')).toBeInTheDocument();
  });

  it('calls onDone(true) when Yes is clicked', () => {
    const onDone = vi.fn();
    render(<ApproveApiKeyDialog customApiKeyTruncated="abc123" onDone={onDone} />);
    fireEvent.click(screen.getByTestId('approve-api-key-yes'));
    expect(onDone).toHaveBeenCalledWith(true);
  });

  it('calls onDone(false) when No is clicked', () => {
    const onDone = vi.fn();
    render(<ApproveApiKeyDialog customApiKeyTruncated="abc123" onDone={onDone} />);
    fireEvent.click(screen.getByTestId('approve-api-key-no'));
    expect(onDone).toHaveBeenCalledWith(false);
  });

  it('calls onDone(false) when close button is clicked', () => {
    const onDone = vi.fn();
    render(<ApproveApiKeyDialog customApiKeyTruncated="abc123" onDone={onDone} />);
    fireEvent.click(screen.getByTestId('approve-api-key-close'));
    expect(onDone).toHaveBeenCalledWith(false);
  });
});
