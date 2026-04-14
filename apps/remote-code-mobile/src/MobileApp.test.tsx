import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import MobileApp from './MobileApp';

const mockRuntime = vi.hoisted(() => ({
  initMobileRuntime: vi.fn(),
  resolveRemoteAccessToken: vi.fn<() => string | null>(() => null),
  resolveRemoteBaseUrl: vi.fn<() => string | null>(() => null),
}));

const mockBiometric = vi.hoisted(() => ({
  performBiometricCheck: vi.fn(),
}));

const mockPush = vi.hoisted(() => ({
  initPushNotifications: vi.fn(),
  requestPushPermission: vi.fn(),
  registerPushTokenWithControlPlane: vi.fn(),
}));

const mockDeepLinks = vi.hoisted(() => ({
  initDeepLinks: vi.fn(),
  parsePairingUrl: vi.fn(() => null),
}));

const mockNetwork = vi.hoisted(() => ({
  initNetworkMonitoring: vi.fn(),
  getNetworkStatus: vi.fn(),
  describeConnectionType: vi.fn((value: string) => value),
  onNetworkChange: vi.fn(() => () => undefined),
}));

const mockHaptics = vi.hoisted(() => ({
  hapticSuccess: vi.fn(),
  hapticMedium: vi.fn(),
  hapticWarning: vi.fn(),
  hapticError: vi.fn(),
}));

vi.mock('@remote/RemoteApp', () => ({
  default: () => <div>Remote shell ready</div>,
}));

vi.mock('./lib/runtime', () => mockRuntime);
vi.mock('./native/biometric', () => mockBiometric);
vi.mock('./native/pushNotifications', () => mockPush);
vi.mock('./native/deepLink', () => mockDeepLinks);
vi.mock('./native/network', () => mockNetwork);
vi.mock('./native/haptics', () => mockHaptics);

describe('MobileApp', () => {
  beforeEach(() => {
    mockRuntime.initMobileRuntime.mockResolvedValue(undefined);
    mockRuntime.resolveRemoteBaseUrl.mockReturnValue(null);
    mockRuntime.resolveRemoteAccessToken.mockReturnValue(null);
    mockBiometric.performBiometricCheck.mockResolvedValue(true);
    mockPush.initPushNotifications.mockResolvedValue(undefined);
    mockPush.requestPushPermission.mockResolvedValue(false);
    mockPush.registerPushTokenWithControlPlane.mockResolvedValue(undefined);
    mockDeepLinks.initDeepLinks.mockImplementation(() => undefined);
    mockNetwork.initNetworkMonitoring.mockResolvedValue(undefined);
    mockNetwork.getNetworkStatus.mockResolvedValue({
      connected: true,
      connectionType: 'wifi',
    });
    mockNetwork.describeConnectionType.mockImplementation((value: string) => value);
    mockNetwork.onNetworkChange.mockReturnValue(() => undefined);
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it('initializes native services and renders the shared remote shell', async () => {
    render(<MobileApp />);

    expect(await screen.findByText('Remote shell ready')).toBeInTheDocument();
    expect(mockRuntime.initMobileRuntime).toHaveBeenCalledTimes(1);
    expect(mockNetwork.initNetworkMonitoring).toHaveBeenCalledTimes(1);
    expect(mockPush.initPushNotifications).toHaveBeenCalledTimes(1);
    expect(mockBiometric.performBiometricCheck).toHaveBeenCalledTimes(1);
    expect(mockHaptics.hapticSuccess).toHaveBeenCalledTimes(1);
  });

  it('registers the push token when a trusted session already exists', async () => {
    mockRuntime.resolveRemoteBaseUrl.mockReturnValue('https://remote-code.yz520gzy.top');
    mockRuntime.resolveRemoteAccessToken.mockReturnValue('device-token');

    render(<MobileApp />);

    await screen.findByText('Remote shell ready');
    await waitFor(() => {
      expect(mockPush.registerPushTokenWithControlPlane).toHaveBeenCalledWith(
        'https://remote-code.yz520gzy.top',
        'device-token',
      );
    });
  });

  it('shows an actionable error screen when biometric verification fails', async () => {
    mockBiometric.performBiometricCheck.mockResolvedValue(false);

    render(<MobileApp />);

    expect(await screen.findByText('初始化失败')).toBeInTheDocument();
    expect(screen.getByText('身份验证失败')).toBeInTheDocument();
    expect(mockHaptics.hapticError).toHaveBeenCalledTimes(1);
  });

  it('renders the offline banner when startup detects a disconnected network', async () => {
    mockNetwork.getNetworkStatus.mockResolvedValue({
      connected: false,
      connectionType: 'cellular',
    });

    render(<MobileApp />);

    expect(await screen.findByText(/网络已断开/)).toBeInTheDocument();
    expect(screen.getByText(/cellular/)).toBeInTheDocument();
  });
});
