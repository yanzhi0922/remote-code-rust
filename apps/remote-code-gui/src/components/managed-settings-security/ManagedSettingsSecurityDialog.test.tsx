import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ManagedSettingsSecurityDialog } from './ManagedSettingsSecurityDialog';

afterEach(() => {
  cleanup();
});

describe('ManagedSettingsSecurityDialog', () => {
  it('renders nothing when closed', () => {
    const { container } = render(
      <ManagedSettingsSecurityDialog settings={{}} open={false} onClose={() => {}} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('shows security pass for safe settings', () => {
    render(<ManagedSettingsSecurityDialog settings={{}} open={true} onClose={() => {}} />);
    expect(screen.getByTestId('ms-security-pass')).toBeInTheDocument();
  });

  it('shows security fail for unsafe settings', () => {
    render(
      <ManagedSettingsSecurityDialog
        settings={{ permissions: { defaultMode: 'bypass' } }}
        open={true}
        onClose={() => {}}
      />,
    );
    expect(screen.getByTestId('ms-security-fail')).toBeInTheDocument();
  });

  it('calls onClose when backdrop clicked', () => {
    const onClose = vi.fn();
    render(<ManagedSettingsSecurityDialog settings={{}} open={true} onClose={onClose} />);
    fireEvent.click(screen.getByTestId('ms-security-backdrop'));
    expect(onClose).toHaveBeenCalled();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    render(<ManagedSettingsSecurityDialog settings={{}} open={true} onClose={onClose} />);
    fireEvent.click(screen.getByTestId('ms-security-close'));
    expect(onClose).toHaveBeenCalled();
  });

  it('shows apply button for safe settings', () => {
    render(
      <ManagedSettingsSecurityDialog
        settings={{}}
        open={true}
        onClose={() => {}}
        onApply={() => {}}
      />,
    );
    expect(screen.getByTestId('ms-security-apply')).toBeInTheDocument();
  });

  it('hides apply button for unsafe settings', () => {
    render(
      <ManagedSettingsSecurityDialog
        settings={{ permissions: { defaultMode: 'bypass' } }}
        open={true}
        onClose={() => {}}
        onApply={() => {}}
      />,
    );
    expect(screen.queryByTestId('ms-security-apply')).not.toBeInTheDocument();
  });
});
