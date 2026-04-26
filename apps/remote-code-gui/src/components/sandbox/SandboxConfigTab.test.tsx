import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { SandboxConfigTab, type SandboxConfig } from './SandboxConfigTab';

describe('SandboxConfigTab', () => {
  afterEach(() => { cleanup(); });

  it('shows disabled state when sandbox not enabled', () => {
    const config: SandboxConfig = {
      enabled: false,
      autoAllowBashIfSandboxed: false,
      excludedCommands: [],
      fsReadDenyPaths: [],
      fsWriteAllowPaths: [],
      networkAllowedHosts: [],
      networkDeniedHosts: [],
    };
    const { getByTestId, getByText } = render(<SandboxConfigTab config={config} />);
    expect(getByTestId('sandbox-config-tab')).toBeInTheDocument();
    expect(getByText('Sandbox is not enabled')).toBeInTheDocument();
  });

  it('shows enabled state with sandbox details', () => {
    const config: SandboxConfig = {
      enabled: true,
      autoAllowBashIfSandboxed: true,
      excludedCommands: ['rm'],
      fsReadDenyPaths: ['/etc/shadow'],
      fsWriteAllowPaths: ['/tmp'],
      networkAllowedHosts: ['api.example.com'],
      networkDeniedHosts: ['evil.com'],
    };
    const { getByText, container } = render(<SandboxConfigTab config={config} />);
    expect(getByText('Sandbox Enabled')).toBeInTheDocument();
    expect(getByText('Auto-allow')).toBeInTheDocument();
    expect(getByText('rm')).toBeInTheDocument();
    // Check that paths are rendered somewhere in the container
    expect(container.textContent).toContain('/etc/shadow');
    expect(container.textContent).toContain('/tmp');
    expect(container.textContent).toContain('api.example.com');
    expect(container.textContent).toContain('evil.com');
  });

  it('shows warnings', () => {
    const config: SandboxConfig = {
      enabled: true,
      autoAllowBashIfSandboxed: false,
      excludedCommands: [],
      fsReadDenyPaths: [],
      fsWriteAllowPaths: [],
      networkAllowedHosts: [],
      networkDeniedHosts: [],
    };
    const { getByText } = render(<SandboxConfigTab config={config} warnings={['Test warning']} />);
    expect(getByText('Test warning')).toBeInTheDocument();
  });
});
