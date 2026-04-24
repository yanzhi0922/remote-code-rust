import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { ChannelDowngradeDialog } from './ChannelDowngradeDialog';

afterEach(() => {
  cleanup();
});

describe('ChannelDowngradeDialog', () => {
  it('renders with data-testid', () => {
    render(<ChannelDowngradeDialog currentVersion="1.0.0" onChoice={vi.fn()} />);
    expect(screen.getByTestId('channel-downgrade-dialog')).toBeInTheDocument();
  });

  it('shows the current version', () => {
    render(<ChannelDowngradeDialog currentVersion="2.5.0" onChoice={vi.fn()} />);
    expect(screen.getAllByText(/2\.5\.0/).length).toBeGreaterThan(0);
  });

  it('shows title', () => {
    render(<ChannelDowngradeDialog currentVersion="1.0.0" onChoice={vi.fn()} />);
    expect(screen.getByText('Switch to Stable Channel')).toBeInTheDocument();
  });

  it('calls onChoice with downgrade when allow button is clicked', () => {
    const onChoice = vi.fn();
    render(<ChannelDowngradeDialog currentVersion="1.0.0" onChoice={onChoice} />);
    fireEvent.click(screen.getByTestId('channel-downgrade-allow'));
    expect(onChoice).toHaveBeenCalledWith('downgrade');
  });

  it('calls onChoice with stay when stay button is clicked', () => {
    const onChoice = vi.fn();
    render(<ChannelDowngradeDialog currentVersion="1.0.0" onChoice={onChoice} />);
    fireEvent.click(screen.getByTestId('channel-downgrade-stay'));
    expect(onChoice).toHaveBeenCalledWith('stay');
  });

  it('calls onChoice with cancel when cancel button is clicked', () => {
    const onChoice = vi.fn();
    render(<ChannelDowngradeDialog currentVersion="1.0.0" onChoice={onChoice} />);
    fireEvent.click(screen.getByTestId('channel-downgrade-cancel'));
    expect(onChoice).toHaveBeenCalledWith('cancel');
  });
});
