import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { RemoteSessionDetailDialog } from './RemoteSessionDetailDialog';

describe('RemoteSessionDetailDialog', () => {
  afterEach(cleanup);

  it('returns null when visible is false', () => {
    render(
      <RemoteSessionDetailDialog
        visible={false}
        sessionId="s1"
        serverUrl="ws://localhost"
        status="running"
        onClose={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('remote-session-detail')).toBeNull();
  });

  it('renders dialog when visible', () => {
    render(
      <RemoteSessionDetailDialog
        visible={true}
        sessionId="sess-123"
        serverUrl="ws://host:8080"
        status="running"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByTestId('remote-session-detail')).toBeInTheDocument();
  });

  it('shows session ID and server URL', () => {
    render(
      <RemoteSessionDetailDialog
        visible={true}
        sessionId="sess-abc"
        serverUrl="ws://myhost:9090"
        status="completed"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText('sess-abc')).toBeInTheDocument();
    expect(screen.getByText('ws://myhost:9090')).toBeInTheDocument();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    render(
      <RemoteSessionDetailDialog
        visible={true}
        sessionId="s1"
        serverUrl="ws://localhost"
        status="running"
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByLabelText('关闭'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('applies custom className', () => {
    render(
      <RemoteSessionDetailDialog
        visible={true}
        sessionId="s1"
        serverUrl="ws://localhost"
        status="running"
        onClose={vi.fn()}
        className="my-class"
      />,
    );
    expect(screen.getByTestId('remote-session-detail').className).toContain('my-class');
  });

  it('shows status badge', () => {
    render(
      <RemoteSessionDetailDialog
        visible={true}
        sessionId="s1"
        serverUrl="ws://localhost"
        status="failed"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText('Failed')).toBeInTheDocument();
  });
});
