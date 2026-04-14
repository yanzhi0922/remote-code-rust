import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { getRemoteCopy } from './i18n';
import { RemoteAuthGate } from './RemoteAuthGate';

describe('RemoteAuthGate', () => {
  it('renders Chinese copy and forwards auth actions', () => {
    const onBootstrapClaim = vi.fn();
    const onPairingAccept = vi.fn();
    const onManualTokenSave = vi.fn();
    const onClearSavedToken = vi.fn();
    const setDeviceName = vi.fn();
    const setBootstrapSecret = vi.fn();
    const setPairingOfferId = vi.fn();
    const setPairingSecret = vi.fn();
    const setManualAccessToken = vi.fn();

    render(
      <RemoteAuthGate
        authErrorMessage="HTTP 401"
        authLoading={false}
        bootstrapEnabled
        copy={getRemoteCopy('zh-CN')}
        deviceName="我的手机"
        health={{
          ok: true,
          service: 'remote-code-control-plane',
          phase: 'phase4',
          runner_count: 1,
          available_runner_count: 1,
          session_count: 2,
          artifact_count: 0,
          queued_runner_command_count: 0,
          auth_required: true,
          bootstrap_secret_configured: true,
          owner_claimed: false,
          device_count: 1,
        }}
        manualAccessToken=""
        onBootstrapClaim={onBootstrapClaim}
        onClearSavedToken={onClearSavedToken}
        onManualTokenSave={onManualTokenSave}
        onPairingAccept={onPairingAccept}
        pairingOfferId=""
        pairingSecret=""
        setBootstrapSecret={setBootstrapSecret}
        setDeviceName={setDeviceName}
        setManualAccessToken={setManualAccessToken}
        setPairingOfferId={setPairingOfferId}
        setPairingSecret={setPairingSecret}
      />,
    );

    expect(screen.getByText('验证当前设备')).toBeInTheDocument();
    expect(screen.getByText('HTTP 401')).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText('我的手机'), {
      target: { value: '安卓 Edge' },
    });
    fireEvent.click(screen.getByRole('button', { name: '认领 Owner 设备' }));
    fireEvent.click(screen.getByRole('button', { name: '接受配对邀请' }));
    fireEvent.click(screen.getByRole('button', { name: '保存令牌' }));
    fireEvent.click(screen.getByRole('button', { name: '清除已保存令牌' }));

    expect(setDeviceName).toHaveBeenCalledWith('安卓 Edge');
    expect(onBootstrapClaim).toHaveBeenCalledTimes(1);
    expect(onPairingAccept).toHaveBeenCalledTimes(1);
    expect(onManualTokenSave).toHaveBeenCalledTimes(1);
    expect(onClearSavedToken).toHaveBeenCalledTimes(1);
  });
});
