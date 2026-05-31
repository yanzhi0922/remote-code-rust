import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import * as tauri from '../../lib/tauri';
import { SettingInput } from './SettingInput';

type RemoteStatus = 'disabled' | 'enabled' | 'running';

interface ConnectionInfo {
  control_plane_url: string;
  runner_id: string;
  auto_start: boolean;
  configured: boolean;
  running: boolean;
  connected: boolean;
}

export function RemoteTab() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<RemoteStatus>('disabled');
  const [connectionInfo, setConnectionInfo] = useState<ConnectionInfo | null>(null);
  const [hasPassword, setHasPassword] = useState(false);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [controlPlaneUrl, setControlPlaneUrl] = useState('');
  const [runnerId, setRunnerId] = useState('');
  const [autoStart, setAutoStart] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const loadState = useCallback(async () => {
    try {
      const [s, info, hp, uname] = await Promise.all([
        tauri.remoteGetStatus(),
        tauri.remoteGetConnectionInfo().catch(() => null),
        tauri.remoteHasPassword(),
        tauri.remoteGetUsername(),
      ]);
      setStatus(s as RemoteStatus);
      setConnectionInfo(info as ConnectionInfo | null);
      setHasPassword(hp);
      setUsername(uname ?? '');
      if (info) {
        setControlPlaneUrl(info.control_plane_url ?? '');
        setRunnerId(info.runner_id ?? '');
        setAutoStart(info.auto_start ?? true);
      }
    } catch {
      // Remote runner may not be initialized yet.
    }
  }, []);

  useEffect(() => {
    void loadState();
  }, [loadState]);

  const handleSaveCredentials = async () => {
    setSaving(true);
    setError(null);
    setSuccess(null);
    try {
      if (username && password) {
        await tauri.remoteSetCredentials(username, password);
      } else if (password) {
        await tauri.remoteSetPassword(password);
      } else if (username) {
        await tauri.remoteSetUsername(username);
      }
      setSuccess(t('remoteTab.credentialsSaved'));
      setPassword('');
      await loadState();
    } catch (e) {
      setError(typeof e === 'string' ? e : String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleSaveConnection = async () => {
    setSaving(true);
    setError(null);
    setSuccess(null);
    try {
      await tauri.remoteSetConnection(controlPlaneUrl, runnerId || undefined, autoStart);
      setSuccess(t('remoteTab.connectionSaved'));
      await loadState();
    } catch (e) {
      setError(typeof e === 'string' ? e : String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleStartService = async () => {
    setSaving(true);
    setError(null);
    try {
      const newStatus = await tauri.remoteStartService();
      setStatus(newStatus as RemoteStatus);
      await loadState();
    } catch (e) {
      setError(typeof e === 'string' ? e : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-6" data-testid="remote-settings">
      <div>
        <h3 className="text-sm font-semibold text-rc-text-primary">{t('settings.remote')}</h3>
        <p className="mt-1 text-xs leading-5 text-rc-text-tertiary">
          {t('remoteTab.description')}
        </p>
      </div>

      {/* Status */}
      <div className="flex items-center gap-2">
        <span className={`inline-block h-2.5 w-2.5 rounded-full ${status === 'running' ? 'bg-rc-accent-success' : status === 'enabled' ? 'bg-rc-accent-warning' : 'bg-rc-text-tertiary'}`} />
        <span className="text-sm text-rc-text-primary capitalize">{status}</span>
        {connectionInfo?.connected && (
          <span className="text-xs text-rc-accent-success ml-2">{t('remoteTab.connected')}</span>
        )}
      </div>

      {/* Error / Success */}
      {error && <p className="text-sm text-rc-accent-error">{error}</p>}
      {success && <p className="text-sm text-rc-accent-success">{success}</p>}

      {/* Connection settings */}
      <div className="space-y-3">
        <h4 className="text-sm font-medium text-rc-text-primary">{t('remoteTab.controlPlane')}</h4>
        <SettingInput
          label={t('remoteTab.controlPlaneUrl')}
          value={controlPlaneUrl}
          onChange={setControlPlaneUrl}
          placeholder={t('remoteTab.controlPlaneUrlPlaceholder')}
        />
        <SettingInput
          label={t('remoteTab.runnerId')}
          value={runnerId}
          onChange={setRunnerId}
          placeholder={t('remoteTab.runnerIdPlaceholder')}
        />
        <label className="flex items-center gap-2 text-sm text-rc-text-primary">
          <input
            type="checkbox"
            checked={autoStart}
            onChange={(e) => setAutoStart(e.target.checked)}
            className="rounded border-rc-border-primary"
          />
          {t('remoteTab.autoStart')}
        </label>
        <button
          onClick={() => void handleSaveConnection()}
          disabled={saving}
          className="rounded-md bg-rc-bg-accent px-3 py-1.5 text-sm text-rc-text-on-accent hover:opacity-90 disabled:opacity-50"
        >
          {saving ? t('common.saving') : t('remoteTab.saveConnection')}
        </button>
      </div>

      {/* Credentials */}
      <div className="space-y-3">
        <h4 className="text-sm font-medium text-rc-text-primary">{t('remoteTab.authentication')}</h4>
        <SettingInput
          label={t('remoteTab.username')}
          value={username}
          onChange={setUsername}
          placeholder="admin"
        />
        <SettingInput
          label={hasPassword ? t('remoteTab.newPassword') : t('remoteTab.password')}
          value={password}
          onChange={setPassword}
          placeholder={hasPassword ? t('remoteTab.passwordSetPlaceholder') : t('remoteTab.passwordNewPlaceholder')}
          type="password"
        />
        <button
          onClick={() => void handleSaveCredentials()}
          disabled={saving || (!username && !password)}
          className="rounded-md bg-rc-bg-accent px-3 py-1.5 text-sm text-rc-text-on-accent hover:opacity-90 disabled:opacity-50"
        >
          {saving ? t('common.saving') : t('remoteTab.saveCredentials')}
        </button>
      </div>

      {/* Start/Stop */}
      {status !== 'running' && (
        <button
          onClick={() => void handleStartService()}
          disabled={saving}
          className="flex items-center gap-2 rounded-md bg-rc-accent-success px-4 py-2 text-sm text-white hover:opacity-90 disabled:opacity-50"
        >
          {t('remoteTab.startService')}
        </button>
      )}
    </div>
  );
}
