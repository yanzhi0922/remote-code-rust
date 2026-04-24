import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { BypassPermissions } from './BypassPermissions';

describe('BypassPermissions', () => {
  afterEach(cleanup);

  it('shows enabled state correctly', () => {
    render(<BypassPermissions enabled={true} onToggle={vi.fn()} killswitchActive={false} />);
    expect(screen.getByText('权限已绕过')).toBeInTheDocument();
  });

  it('shows disabled state correctly', () => {
    render(<BypassPermissions enabled={false} onToggle={vi.fn()} killswitchActive={false} />);
    expect(screen.getByText('权限模式已启用')).toBeInTheDocument();
  });

  it('shows confirmation dialog when enabling bypass', () => {
    render(<BypassPermissions enabled={false} onToggle={vi.fn()} killswitchActive={false} />);
    fireEvent.click(screen.getByRole('switch'));
    expect(screen.getByText('确认绕过权限？')).toBeInTheDocument();
  });

  it('calls onToggle directly when disabling bypass', () => {
    const onToggle = vi.fn();
    render(<BypassPermissions enabled={true} onToggle={onToggle} killswitchActive={false} />);
    fireEvent.click(screen.getByRole('switch'));
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it('calls onToggle after confirming enable', () => {
    const onToggle = vi.fn();
    render(<BypassPermissions enabled={false} onToggle={onToggle} killswitchActive={false} />);
    fireEvent.click(screen.getByRole('switch'));
    fireEvent.click(screen.getByText('确认绕过'));
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it('cancels confirmation without toggling', () => {
    const onToggle = vi.fn();
    render(<BypassPermissions enabled={false} onToggle={onToggle} killswitchActive={false} />);
    fireEvent.click(screen.getByRole('switch'));
    fireEvent.click(screen.getByText('取消'));
    expect(onToggle).not.toHaveBeenCalled();
    expect(screen.queryByText('确认绕过权限？')).toBeNull();
  });

  it('shows killswitch warning when active', () => {
    render(<BypassPermissions enabled={false} onToggle={vi.fn()} killswitchActive={true} />);
    expect(screen.getByText(/紧急停止已激活/)).toBeInTheDocument();
  });

  it('disables toggle when killswitch is active', () => {
    render(<BypassPermissions enabled={false} onToggle={vi.fn()} killswitchActive={true} />);
    const toggle = screen.getByRole('switch');
    expect(toggle).toBeDisabled();
  });
});
