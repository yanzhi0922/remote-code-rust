import { afterEach, describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { SandboxSettings } from './SandboxSettings';
import { SandboxConfigTab } from './SandboxConfigTab';
import { SandboxDependenciesTab } from './SandboxDependenciesTab';
import { SandboxOverridesTab } from './SandboxOverridesTab';

afterEach(() => {
  cleanup();
});

const defaultConfig = {
  enabled: true,
  autoAllowBashIfSandboxed: false,
  excludedCommands: [] as string[],
  fsReadDenyPaths: [] as string[],
  fsWriteAllowPaths: [] as string[],
  networkAllowedHosts: [] as string[],
  networkDeniedHosts: [] as string[],
};

const defaultDepCheck = {
  errors: [] as string[],
  warnings: [] as string[],
};

describe('SandboxSettings', () => {
  it('renders with tabs', () => {
    render(<SandboxSettings config={defaultConfig} depCheck={defaultDepCheck} />);
    expect(screen.getByTestId('sandbox-settings')).toBeInTheDocument();
    expect(screen.getByTestId('sandbox-tab-config')).toBeInTheDocument();
    expect(screen.getByTestId('sandbox-tab-dependencies')).toBeInTheDocument();
    expect(screen.getByTestId('sandbox-tab-overrides')).toBeInTheDocument();
  });

  it('switches tabs on click', () => {
    render(<SandboxSettings config={defaultConfig} depCheck={defaultDepCheck} />);
    fireEvent.click(screen.getByTestId('sandbox-tab-dependencies'));
    expect(screen.getByTestId('sandbox-dependencies-tab')).toBeInTheDocument();
  });

  it('shows overrides tab', () => {
    render(<SandboxSettings config={defaultConfig} depCheck={defaultDepCheck} />);
    fireEvent.click(screen.getByTestId('sandbox-tab-overrides'));
    expect(screen.getByTestId('sandbox-overrides-tab')).toBeInTheDocument();
  });

  it('calls onModeChange when clicking mode buttons', () => {
    const onModeChange = vi.fn();
    render(
      <SandboxSettings
        config={defaultConfig}
        depCheck={defaultDepCheck}
        onModeChange={onModeChange}
      />,
    );
    fireEvent.click(screen.getByTestId('sandbox-mode-auto-allow'));
    expect(onModeChange).toHaveBeenCalledWith('auto-allow');
  });

  it('shows config tab by default', () => {
    render(<SandboxSettings config={defaultConfig} depCheck={defaultDepCheck} />);
    expect(screen.getByTestId('sandbox-config-tab')).toBeInTheDocument();
  });
});

describe('SandboxConfigTab', () => {
  it('shows not enabled message when disabled', () => {
    render(<SandboxConfigTab config={{ ...defaultConfig, enabled: false }} />);
    expect(screen.getByText('Sandbox is not enabled')).toBeInTheDocument();
  });

  it('shows enabled state with config details', () => {
    render(
      <SandboxConfigTab
        config={{
          ...defaultConfig,
          enabled: true,
          excludedCommands: ['rm', 'mkfs'],
        }}
      />,
    );
    expect(screen.getByText('Sandbox Enabled')).toBeInTheDocument();
    expect(screen.getByText('rm, mkfs')).toBeInTheDocument();
  });

  it('shows auto-allow badge', () => {
    render(
      <SandboxConfigTab
        config={{ ...defaultConfig, enabled: true, autoAllowBashIfSandboxed: true }}
      />,
    );
    expect(screen.getByText('Auto-allow')).toBeInTheDocument();
  });

  it('shows warnings', () => {
    render(
      <SandboxConfigTab
        config={{ ...defaultConfig, enabled: true }}
        warnings={['seccomp not available']}
      />,
    );
    expect(screen.getByText('seccomp not available')).toBeInTheDocument();
  });

  it('shows network restrictions', () => {
    render(
      <SandboxConfigTab
        config={{
          ...defaultConfig,
          enabled: true,
          networkAllowedHosts: ['api.example.com'],
          networkDeniedHosts: ['evil.com'],
        }}
      />,
    );
    expect(screen.getByText(/api\.example\.com/)).toBeInTheDocument();
    expect(screen.getByText(/evil\.com/)).toBeInTheDocument();
  });
});

describe('SandboxDependenciesTab', () => {
  it('shows found status for all deps when no errors', () => {
    render(<SandboxDependenciesTab depCheck={defaultDepCheck} platform="linux" />);
    expect(screen.getByTestId('sandbox-dependencies-tab')).toBeInTheDocument();
    expect(screen.getByText('found')).toBeInTheDocument();
  });

  it('shows not found for ripgrep when error includes ripgrep', () => {
    render(
      <SandboxDependenciesTab
        depCheck={{ errors: ['ripgrep not found'], warnings: [] }}
        platform="linux"
      />,
    );
    expect(screen.getByText('not found')).toBeInTheDocument();
  });

  it('shows macOS seatbelt as built-in', () => {
    render(<SandboxDependenciesTab depCheck={defaultDepCheck} platform="macos" />);
    expect(screen.getByText('built-in (macOS)')).toBeInTheDocument();
  });

  it('shows other errors', () => {
    render(
      <SandboxDependenciesTab
        depCheck={{ errors: ['custom error'], warnings: [] }}
        platform="linux"
      />,
    );
    expect(screen.getByText('custom error')).toBeInTheDocument();
  });

  it('does not show linux-specific deps on macOS', () => {
    render(
      <SandboxDependenciesTab
        depCheck={{ errors: ['bwrap missing'], warnings: [] }}
        platform="macos"
      />,
    );
    expect(screen.queryByText('bubblewrap (bwrap):')).not.toBeInTheDocument();
  });
});

describe('SandboxOverridesTab', () => {
  it('shows not enabled message', () => {
    render(
      <SandboxOverridesTab
        isEnabled={false}
        isLocked={false}
        currentAllowUnsandboxed={false}
      />,
    );
    expect(screen.getByText(/Sandbox is not enabled/)).toBeInTheDocument();
  });

  it('shows locked message', () => {
    render(
      <SandboxOverridesTab
        isEnabled={true}
        isLocked={true}
        currentAllowUnsandboxed={false}
      />,
    );
    expect(screen.getByText(/managed by a higher-priority/)).toBeInTheDocument();
  });

  it('shows mode buttons when enabled and unlocked', () => {
    render(
      <SandboxOverridesTab
        isEnabled={true}
        isLocked={false}
        currentAllowUnsandboxed={false}
      />,
    );
    expect(screen.getByTestId('override-mode-open')).toBeInTheDocument();
    expect(screen.getByTestId('override-mode-closed')).toBeInTheDocument();
  });

  it('calls onModeChange when clicking mode button', () => {
    const onModeChange = vi.fn();
    render(
      <SandboxOverridesTab
        isEnabled={true}
        isLocked={false}
        currentAllowUnsandboxed={false}
        onModeChange={onModeChange}
      />,
    );
    fireEvent.click(screen.getByTestId('override-mode-open'));
    expect(onModeChange).toHaveBeenCalledWith('open');
  });

  it('shows current indicator on active mode', () => {
    render(
      <SandboxOverridesTab
        isEnabled={true}
        isLocked={false}
        currentAllowUnsandboxed={true}
      />,
    );
    expect(screen.getByText('(current)')).toBeInTheDocument();
  });
});
