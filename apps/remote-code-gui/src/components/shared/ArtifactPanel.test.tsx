import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ArtifactPanel, type ArtifactPanelProps } from './ArtifactPanel';

afterEach(() => { cleanup(); });

const baseProps: ArtifactPanelProps = {
  title: 'Artifacts',
  icon: <span>📦</span>,
  emptyText: 'No artifacts yet',
  items: [],
};

describe('ArtifactPanel', () => {
  it('renders title', () => {
    render(<ArtifactPanel {...baseProps} />);
    expect(screen.getByText('Artifacts')).toBeInTheDocument();
  });

  it('shows empty text when no items', () => {
    render(<ArtifactPanel {...baseProps} />);
    expect(screen.getByText('No artifacts yet')).toBeInTheDocument();
  });

  it('renders artifact items', () => {
    render(
      <ArtifactPanel
        {...baseProps}
        items={[{ artifact_id: '1', name: 'Build', file_name: 'app.tar.gz', size_bytes: 1024 }]}
      />,
    );
    expect(screen.getByText('Build')).toBeInTheDocument();
  });

  it('calls onDownload when download button is clicked', async () => {
    const onDownload = vi.fn();
    render(
      <ArtifactPanel
        {...baseProps}
        items={[{ artifact_id: '1', name: 'Build', file_name: 'app.tar.gz', size_bytes: 1024 }]}
        onDownload={onDownload}
      />,
    );
    await userEvent.click(screen.getByRole('button', { name: /Build/i }));
    expect(onDownload).toHaveBeenCalledWith(
      expect.objectContaining({ artifact_id: '1', name: 'Build' }),
    );
  });

  it('disables download button when downloadingId matches', () => {
    render(
      <ArtifactPanel
        {...baseProps}
        items={[{ artifact_id: '1', name: 'Build', file_name: 'app.tar.gz', size_bytes: 1024 }]}
        downloadingId="1"
        onDownload={() => {}}
      />,
    );
    expect(screen.getByRole('button', { name: /Build/i })).toBeDisabled();
  });
});
