import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { DevChannelsDialog } from './DevChannelsDialog';

afterEach(() => {
  cleanup();
});

describe('DevChannelsDialog', () => {
  const channels = [
    { kind: 'server' as const, name: 'test-server' },
    { kind: 'plugin' as const, name: 'my-plugin', marketplace: 'official' },
  ];

  it('renders with data-testid', () => {
    render(<DevChannelsDialog channels={channels} onAccept={vi.fn()} />);
    expect(screen.getByTestId('dev-channels-dialog')).toBeInTheDocument();
  });

  it('shows warning title', () => {
    render(<DevChannelsDialog channels={channels} onAccept={vi.fn()} />);
    expect(screen.getByText(/WARNING: Loading development channels/)).toBeInTheDocument();
  });

  it('shows channel names', () => {
    render(<DevChannelsDialog channels={channels} onAccept={vi.fn()} />);
    expect(screen.getByText(/server:test-server/)).toBeInTheDocument();
    expect(screen.getByText(/plugin:my-plugin@official/)).toBeInTheDocument();
  });

  it('calls onAccept when accept button is clicked', () => {
    const onAccept = vi.fn();
    render(<DevChannelsDialog channels={channels} onAccept={onAccept} />);
    fireEvent.click(screen.getByTestId('dev-channels-accept'));
    expect(onAccept).toHaveBeenCalledOnce();
  });

  it('calls onExit when exit button is clicked', () => {
    const onExit = vi.fn();
    render(<DevChannelsDialog channels={channels} onAccept={vi.fn()} onExit={onExit} />);
    fireEvent.click(screen.getByTestId('dev-channels-exit'));
    expect(onExit).toHaveBeenCalledOnce();
  });

  it('calls onExit when close button is clicked', () => {
    const onExit = vi.fn();
    render(<DevChannelsDialog channels={channels} onAccept={vi.fn()} onExit={onExit} />);
    fireEvent.click(screen.getByTestId('dev-channels-close'));
    expect(onExit).toHaveBeenCalledOnce();
  });
});
