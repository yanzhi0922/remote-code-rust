import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { BypassPermissionsModeDialog } from './BypassPermissionsModeDialog';

afterEach(() => {
  cleanup();
});

describe('BypassPermissionsModeDialog', () => {
  it('renders nothing when closed', () => {
    const { container } = render(<BypassPermissionsModeDialog open={false} onAccept={() => {}} onDecline={() => {}} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders warning dialog when open', () => {
    render(<BypassPermissionsModeDialog open={true} onAccept={() => {}} onDecline={() => {}} />);
    expect(screen.getByTestId('bypass-permissions-dialog')).toBeInTheDocument();
    expect(screen.getByText('警告: 绕过权限模式')).toBeInTheDocument();
  });

  it('calls onAccept', () => {
    const onAccept = vi.fn();
    render(<BypassPermissionsModeDialog open={true} onAccept={onAccept} onDecline={() => {}} />);
    fireEvent.click(screen.getByTestId('bypass-accept'));
    expect(onAccept).toHaveBeenCalled();
  });

  it('calls onDecline', () => {
    const onDecline = vi.fn();
    render(<BypassPermissionsModeDialog open={true} onAccept={() => {}} onDecline={onDecline} />);
    fireEvent.click(screen.getByTestId('bypass-decline'));
    expect(onDecline).toHaveBeenCalled();
  });
});
