import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
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
});
