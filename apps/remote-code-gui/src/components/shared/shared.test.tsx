import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApprovalPanel, PanelHint } from './ApprovalPanel';
import { ArtifactPanel } from './ArtifactPanel';
import { TimelineEventCard } from './TimelineEventCard';
import { TimelineMessageCard } from './TimelineMessageCard';

afterEach(() => {
  cleanup();
});

describe('TimelineMessageCard', () => {
  it('renders user message with right-aligned dark bubble', () => {
    render(
      <TimelineMessageCard role="user" header="You">
        Hello world
      </TimelineMessageCard>,
    );
    const card = screen.getByText('Hello world').closest('div')!.parentElement!;
    expect(card.className).toContain('justify-end');
    expect(screen.getByText('You')).toBeInTheDocument();
  });

  it('renders assistant message with left-aligned light bubble', () => {
    render(
      <TimelineMessageCard role="assistant" header="Assistant">
        Response text
      </TimelineMessageCard>,
    );
    const textElement = screen.getByText('Response text');
    const flexContainer = textElement.closest('.flex');
    expect(flexContainer!.className).toContain('justify-start');
    expect(screen.getByText('Assistant')).toBeInTheDocument();
  });

  it('renders system role messages', () => {
    render(
      <TimelineMessageCard role="system" header="System">
        System notification
      </TimelineMessageCard>,
    );
    expect(screen.getByText('System notification')).toBeInTheDocument();
    expect(screen.getByText('System')).toBeInTheDocument();
  });
});

describe('TimelineEventCard', () => {
  it('renders eyebrow, timestamp, and children', () => {
    render(
      <TimelineEventCard
        eyebrow="Tool"
        accent="text-emerald-700"
        icon={<span data-testid="icon">🔧</span>}
        timestampLabel="2 min ago"
      >
        <span>Tool ran successfully</span>
      </TimelineEventCard>,
    );
    expect(screen.getByText('Tool')).toBeInTheDocument();
    expect(screen.getByText('2 min ago')).toBeInTheDocument();
    expect(screen.getByText('Tool ran successfully')).toBeInTheDocument();
    expect(screen.getByTestId('icon')).toBeInTheDocument();
  });
});

describe('ApprovalPanel', () => {
  const items = [
    {
      approval_id: 'apr-1',
      title: 'Delete file',
      description: 'Request to delete /tmp/test.log',
      metadata: { blocked_path: '/tmp/test.log' },
    },
  ];

  const actions = [
    { decision: 'approved', label: 'Allow', className: 'bg-green-600 text-white' },
    { decision: 'denied', label: 'Deny', className: 'bg-red-600 text-white' },
  ];

  it('renders approval items with action buttons', async () => {
    const onDecision = vi.fn();
    render(
      <ApprovalPanel
        title="Approvals"
        icon={<span>🛡️</span>}
        emptyText="No approvals"
        items={items}
        actions={actions}
        approvingId={null}
        loadingText="Loading..."
        onDecision={onDecision}
      />,
    );

    expect(screen.getByText('Approvals')).toBeInTheDocument();
    expect(screen.getByText('Delete file')).toBeInTheDocument();
    expect(screen.getByText('/tmp/test.log')).toBeInTheDocument();

    await userEvent.click(screen.getByText('Allow'));
    expect(onDecision).toHaveBeenCalledWith('apr-1', 'approved');
  });

  it('shows empty text when no items', () => {
    render(
      <ApprovalPanel
        title="Approvals"
        icon={<span>🛡️</span>}
        emptyText="Nothing pending"
        items={[]}
        actions={actions}
        approvingId={null}
        loadingText="Loading..."
        onDecision={() => {}}
      />,
    );
    expect(screen.getByText('Nothing pending')).toBeInTheDocument();
  });

  it('shows loading spinner when approving', () => {
    render(
      <ApprovalPanel
        title="Approvals"
        icon={<span>🛡️</span>}
        emptyText="No approvals"
        items={items}
        actions={actions}
        approvingId="apr-1"
        loadingText="Processing..."
        onDecision={() => {}}
      />,
    );
    // Both action buttons show loading text when approving
    const loadingLabels = screen.getAllByText('Processing...');
    expect(loadingLabels.length).toBeGreaterThanOrEqual(1);
  });
});

describe('ArtifactPanel', () => {
  const items = [
    { artifact_id: 'art-1', name: 'Report', file_name: 'report.pdf', size_bytes: 2048 },
    { artifact_id: 'art-2', name: 'Data', file_name: 'data.csv', size_bytes: 1536 },
  ];

  it('renders artifact items with download actions', async () => {
    const onDownload = vi.fn();
    render(
      <ArtifactPanel
        title="Artifacts"
        icon={<span>📦</span>}
        emptyText="No artifacts"
        items={items}
        onDownload={onDownload}
      />,
    );

    expect(screen.getByText('Artifacts')).toBeInTheDocument();
    expect(screen.getByText('Report')).toBeInTheDocument();
    expect(screen.getByText('Data')).toBeInTheDocument();
    expect(screen.getByText(/data\.csv/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: /Report/i }));
    expect(onDownload).toHaveBeenCalledWith(items[0]);
  });

  it('can still render link fallback mode', () => {
    render(
      <ArtifactPanel
        title="Artifacts"
        icon={<span>📦</span>}
        emptyText="No artifacts"
        items={items}
        buildDownloadUrl={(id) => `https://example.com/download/${id}`}
      />,
    );

    const links = screen.getAllByRole('link');
    expect(links[0]).toHaveAttribute('href', 'https://example.com/download/art-1');
    expect(links[1]).toHaveAttribute('href', 'https://example.com/download/art-2');
  });

  it('shows empty text when no items', () => {
    render(
      <ArtifactPanel
        title="Artifacts"
        icon={<span>📦</span>}
        emptyText="Nothing here"
        items={[]}
        onDownload={() => {}}
      />,
    );
    expect(screen.getByText('Nothing here')).toBeInTheDocument();
  });
});

describe('PanelHint', () => {
  it('renders children as text', () => {
    render(<PanelHint>Hint message</PanelHint>);
    expect(screen.getByText('Hint message')).toBeInTheDocument();
  });
});
