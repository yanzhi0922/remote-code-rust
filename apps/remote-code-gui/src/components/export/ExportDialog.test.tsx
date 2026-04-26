import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { ExportDialog } from './ExportDialog';

describe('ExportDialog', () => {
  afterEach(() => { cleanup(); });

  it('returns null when not visible', () => {
    const { container } = render(
      <ExportDialog visible={false} sessionId="s1" onExport={() => {}} onClose={() => {}} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('renders dialog when visible', () => {
    const { getByTestId, getByText } = render(
      <ExportDialog visible sessionId="s1" onExport={() => {}} onClose={() => {}} />,
    );
    expect(getByTestId('export-dialog')).toBeInTheDocument();
    expect(getByText('导出会话')).toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    const { getByTestId } = render(
      <ExportDialog visible sessionId="s1" onExport={() => {}} onClose={onClose} />,
    );
    fireEvent.click(getByTestId('export-close'));
    expect(onClose).toHaveBeenCalled();
  });

  it('calls onExport with selected format when export button clicked', () => {
    const onExport = vi.fn();
    const { getByTestId } = render(
      <ExportDialog visible sessionId="s1" onExport={onExport} onClose={() => {}} />,
    );
    fireEvent.click(getByTestId('export-button'));
    expect(onExport).toHaveBeenCalledWith('json');
  });
});
