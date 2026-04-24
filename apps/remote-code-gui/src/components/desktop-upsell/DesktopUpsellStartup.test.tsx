import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DesktopUpsellStartup } from './DesktopUpsellStartup';

afterEach(() => {
  cleanup();
});

describe('DesktopUpsellStartup', () => {
  it('renders upsell content', () => {
    render(<DesktopUpsellStartup onDone={() => {}} />);
    expect(screen.getByTestId('desktop-upsell-startup')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '试试桌面版' })).toBeInTheDocument();
  });

  it('calls onDone when not-now clicked', () => {
    const onDone = vi.fn();
    render(<DesktopUpsellStartup onDone={onDone} />);
    fireEvent.click(screen.getByTestId('desktop-upsell-not-now'));
    expect(onDone).toHaveBeenCalled();
  });

  it('calls onTryDesktop when try clicked', () => {
    const onTryDesktop = vi.fn();
    render(<DesktopUpsellStartup onDone={() => {}} onTryDesktop={onTryDesktop} />);
    fireEvent.click(screen.getByTestId('desktop-upsell-try'));
    expect(onTryDesktop).toHaveBeenCalled();
  });

  it('calls onDone when never clicked', () => {
    const onDone = vi.fn();
    render(<DesktopUpsellStartup onDone={onDone} />);
    fireEvent.click(screen.getByTestId('desktop-upsell-never'));
    expect(onDone).toHaveBeenCalled();
  });

  it('calls onDone when close clicked', () => {
    const onDone = vi.fn();
    render(<DesktopUpsellStartup onDone={onDone} />);
    fireEvent.click(screen.getByTestId('desktop-upsell-close'));
    expect(onDone).toHaveBeenCalled();
  });
});
