import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AutoModeOptInDialog } from './AutoModeOptInDialog';

afterEach(() => {
  cleanup();
});

describe('AutoModeOptInDialog', () => {
  it('renders nothing when closed', () => {
    const { container } = render(<AutoModeOptInDialog open={false} onAccept={() => {}} onDecline={() => {}} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders dialog when open', () => {
    render(<AutoModeOptInDialog open={true} onAccept={() => {}} onDecline={() => {}} />);
    expect(screen.getByTestId('auto-mode-opt-in-dialog')).toBeInTheDocument();
  });

  it('calls onAccept', () => {
    const onAccept = vi.fn();
    render(<AutoModeOptInDialog open={true} onAccept={onAccept} onDecline={() => {}} />);
    fireEvent.click(screen.getByTestId('auto-mode-accept'));
    expect(onAccept).toHaveBeenCalled();
  });

  it('calls onDecline', () => {
    const onDecline = vi.fn();
    render(<AutoModeOptInDialog open={true} onAccept={() => {}} onDecline={onDecline} />);
    fireEvent.click(screen.getByTestId('auto-mode-decline'));
    expect(onDecline).toHaveBeenCalled();
  });

  it('shows exit text when declineExits', () => {
    render(<AutoModeOptInDialog open={true} onAccept={() => {}} onDecline={() => {}} declineExits={true} />);
    expect(screen.getByText('退出')).toBeInTheDocument();
  });
});
