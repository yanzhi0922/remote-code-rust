import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ExportDialog } from './ExportDialog';

afterEach(() => {
  cleanup();
});

describe('ExportDialog', () => {
  const sessionId = 'session-abc123def456ghi789';

  it('renders nothing when visible is false', () => {
    render(<ExportDialog visible={false} sessionId={sessionId} onExport={vi.fn()} onClose={vi.fn()} />);
    expect(screen.queryByTestId('export-dialog')).not.toBeInTheDocument();
  });

  it('renders dialog when visible is true', () => {
    render(<ExportDialog visible={true} sessionId={sessionId} onExport={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTestId('export-dialog')).toBeInTheDocument();
    expect(screen.getByText('导出会话')).toBeInTheDocument();
  });

  it('shows session ID', () => {
    render(<ExportDialog visible={true} sessionId={sessionId} onExport={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTestId('export-session-id')).toHaveTextContent('session-abc1');
  });

  it('renders both format options', () => {
    render(<ExportDialog visible={true} sessionId={sessionId} onExport={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTestId('format-json')).toBeInTheDocument();
    expect(screen.getByTestId('format-ndjson')).toBeInTheDocument();
  });

  it('selects format on click', () => {
    render(<ExportDialog visible={true} sessionId={sessionId} onExport={vi.fn()} onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId('format-ndjson'));
    expect(screen.getByTestId('format-check-ndjson')).toBeInTheDocument();
  });

  it('calls onExport with selected format', () => {
    const onExport = vi.fn();
    render(<ExportDialog visible={true} sessionId={sessionId} onExport={onExport} onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId('format-ndjson'));
    fireEvent.click(screen.getByTestId('export-button'));
    expect(onExport).toHaveBeenCalledWith('ndjson');
  });

  it('shows success message after export', () => {
    render(<ExportDialog visible={true} sessionId={sessionId} onExport={vi.fn()} onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId('export-button'));
    expect(screen.getByTestId('export-success')).toBeInTheDocument();
    expect(screen.getByText('导出成功！')).toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    render(<ExportDialog visible={true} sessionId={sessionId} onExport={vi.fn()} onClose={onClose} />);
    fireEvent.click(screen.getByTestId('export-close'));
    expect(onClose).toHaveBeenCalled();
  });

  it('defaults to JSON format', () => {
    render(<ExportDialog visible={true} sessionId={sessionId} onExport={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTestId('format-check-json')).toBeInTheDocument();
  });
});
