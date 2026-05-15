import { useCallback, useEffect, useState } from 'react';
import {
  Eye,
  EyeOff,
  Loader2,
  Play,
  Save,
  ShieldCheck,
  Smartphone,
  User,
  Wifi,
  WifiOff,
} from 'lucide-react';
import * as tauri from '../../lib/tauri';

type RemoteStatus = 'loading' | tauri.RemoteControlStatus;

const DEFAULT_CONTROL_PLANE_URL = 'https://remote-code.yz520gzy.top';

export function RemoteSettings() {
  const [status, setStatus] = useState<RemoteStatus>('loading');
  const [connectionInfo, setConnectionInfo] = useState<tauri.RemoteConnectionInfo | null>(null);
  const [controlPlaneUrl, setControlPlaneUrl] = useState(DEFAULT_CONTROL_PLANE_URL);
  const [runnerId, setRunnerId] = useState('');
  const [autoStart, setAutoStart] = useState(true);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [savingConnection, setSavingConnection] = useState(false);
  const [savingCredentials, setSavingCredentials] = useState(false);
  const [savedConnection, setSavedConnection] = useState(false);
  const [savedCredentials, setSavedCredentials] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hasExistingCredentials, setHasExistingCredentials] = useState(false);

  const refresh = useCallback(async () => {
    setStatus('loading');
    const [nextStatus, info, storedUsername, hasPassword] = await Promise.allSettled([
      tauri.remoteGetStatus(),
      tauri.remoteGetConnectionInfo(),
      tauri.remoteGetUsername(),
      tauri.remoteHasPassword(),
    ]);

    setStatus(nextStatus.status === 'fulfilled' ? nextStatus.value : 'disabled');

    if (info.status === 'fulfilled') {
      setConnectionInfo(info.value);
      setControlPlaneUrl(info.value.control_plane_url || DEFAULT_CONTROL_PLANE_URL);
      setRunnerId(info.value.runner_id || '');
      setAutoStart(info.value.auto_start);
    } else {
      setConnectionInfo(null);
    }

    if (storedUsername.status === 'fulfilled' && storedUsername.value) {
      setUsername(storedUsername.value);
    }

    setHasExistingCredentials(hasPassword.status === 'fulfilled' ? hasPassword.value : false);
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (status !== 'running') return;
    const timer = window.setInterval(() => {
      void tauri.remoteGetConnectionInfo().then(setConnectionInfo).catch(() => undefined);
    }, 5000);
    return () => window.clearInterval(timer);
  }, [status]);

  const handleSaveConnection = useCallback(async () => {
    if (!controlPlaneUrl.trim()) return;
    if (!hasExistingCredentials && (!username.trim() || password.length < 4)) return;
    const wasRunning = status === 'running';
    setSavingConnection(true);
    setSavedConnection(false);
    setMessage(null);
    setError(null);
    try {
      if (!hasExistingCredentials) {
        await tauri.remoteSetCredentials(username, password);
        setHasExistingCredentials(true);
      }

      const info = await tauri.remoteSetConnection(controlPlaneUrl, runnerId, autoStart);
      setConnectionInfo(info);
      setControlPlaneUrl(info.control_plane_url || '');
      setRunnerId(info.runner_id || '');
      setAutoStart(info.auto_start);

      if (!wasRunning) {
        const nextStatus = await tauri.remoteStartService();
        setStatus(nextStatus);
        setConnectionInfo((current) => current ? { ...current, running: nextStatus === 'running' } : current);
        setMessage('连接已保存，远程服务已启动。');
      } else {
        setStatus('running');
        setMessage('连接已保存；当前服务已在运行，重启应用后会使用新配置。');
      }

      setSavedConnection(true);
      setTimeout(() => setSavedConnection(false), 2000);
    } catch (e) {
      setError(e instanceof Error ? e.message : '保存连接失败');
    } finally {
      setSavingConnection(false);
    }
  }, [autoStart, controlPlaneUrl, hasExistingCredentials, password, runnerId, status, username]);

  const handleStartService = useCallback(async () => {
    setSavingConnection(true);
    setMessage(null);
    setError(null);
    try {
      const nextStatus = await tauri.remoteStartService();
      setStatus(nextStatus);
      const info = await tauri.remoteGetConnectionInfo();
      setConnectionInfo(info);
      setMessage(nextStatus === 'running' ? '远程服务已运行。' : '远程服务已配置。');
    } catch (e) {
      setError(e instanceof Error ? e.message : '启动远程服务失败');
    } finally {
      setSavingConnection(false);
    }
  }, []);

  const handleSaveCredentials = useCallback(async () => {
    if (!username.trim() || password.length < 4) return;
    setSavingCredentials(true);
    setSavedCredentials(false);
    setMessage(null);
    setError(null);
    try {
      await tauri.remoteSetCredentials(username, password);
      setHasExistingCredentials(true);
      setSavedCredentials(true);
      setMessage('账户与配对密码已保存。');
      setTimeout(() => setSavedCredentials(false), 2000);
    } catch (e) {
      setError(e instanceof Error ? e.message : '保存账户失败');
    } finally {
      setSavingCredentials(false);
    }
  }, [username, password]);

  const isRunning = status === 'running';
  const isConnected = Boolean(connectionInfo?.connected);
  const isConfigured = status === 'enabled' || status === 'running' || connectionInfo?.configured;
  const canStartWithCredentials =
    hasExistingCredentials || (username.trim().length > 0 && password.length >= 4);

  return (
    <div className="space-y-6">
      <div className="rounded-xl border border-slate-200 bg-slate-50 p-4">
        <div className="flex items-center gap-3">
          {status === 'loading' ? (
            <Loader2 size={20} className="animate-spin text-slate-400" />
          ) : isRunning && isConnected ? (
            <Wifi size={20} className="text-green-600" />
          ) : isRunning ? (
            <Loader2 size={20} className="animate-spin text-blue-500" />
          ) : (
            <WifiOff size={20} className="text-slate-400" />
          )}
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium text-slate-800">
              {status === 'loading'
                ? '检测中...'
                : isRunning && isConnected
                  ? '远程控制运行中'
                  : isRunning
                    ? '正在连接控制平面'
                  : isConfigured
                    ? '远程控制已配置'
                    : '远程控制未配置'}
            </div>
            <p className="truncate text-xs text-slate-500">
              {isConfigured
                ? '手机 App 可通过控制平面连接这台电脑'
                : '填写控制平面地址后可启用手机远程控制'}
            </p>
          </div>
          <span
            className={`inline-flex shrink-0 items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${
              isRunning && isConnected
                ? 'bg-green-100 text-green-700'
                : isRunning
                  ? 'bg-blue-100 text-blue-700'
                : isConfigured
                  ? 'bg-blue-100 text-blue-700'
                  : 'bg-slate-100 text-slate-500'
            }`}
          >
            {status === 'loading' ? '--' : isRunning && isConnected ? 'ONLINE' : isRunning ? 'CONNECTING' : isConfigured ? 'READY' : 'OFF'}
          </span>
        </div>
      </div>

      <section className="space-y-3">
        <div className="flex items-center gap-2">
          <Smartphone size={16} className="text-slate-600" />
          <h3 className="text-sm font-medium text-slate-700">远程连接</h3>
        </div>

        <label className="block space-y-1.5">
          <span className="text-xs font-medium text-slate-600">控制平面 URL</span>
          <input
            type="url"
            value={controlPlaneUrl}
            onChange={(event) => setControlPlaneUrl(event.target.value)}
            placeholder={DEFAULT_CONTROL_PLANE_URL}
            className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
        </label>

        <label className="block space-y-1.5">
          <span className="text-xs font-medium text-slate-600">Runner ID</span>
          <input
            type="text"
            value={runnerId}
            onChange={(event) => setRunnerId(event.target.value)}
            placeholder="留空自动生成"
            className="w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm text-slate-800 placeholder:font-sans placeholder:text-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
        </label>

        <label className="flex items-start gap-3 rounded-lg border border-slate-200 bg-white p-3">
          <input
            type="checkbox"
            checked={autoStart}
            onChange={(event) => setAutoStart(event.target.checked)}
            className="mt-0.5 h-4 w-4 rounded border-slate-300 text-blue-600 focus:ring-blue-500"
          />
          <span className="min-w-0">
            <span className="block text-sm font-medium text-slate-700">随 GUI 自动启动远程服务</span>
            <span className="block text-xs text-slate-500">
              保存后，下次从桌面快捷方式打开应用会自动连接控制平面。
            </span>
          </span>
        </label>

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => void handleSaveConnection()}
            disabled={!controlPlaneUrl.trim() || !canStartWithCredentials || savingConnection}
            className="inline-flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {savingConnection ? <Loader2 size={16} className="animate-spin" /> : <Save size={16} />}
            {savingConnection ? '保存中...' : savedConnection ? '已保存' : '保存并启动'}
          </button>
          <button
            type="button"
            onClick={() => void handleStartService()}
            disabled={!isConfigured || !hasExistingCredentials || isRunning || savingConnection}
            className="inline-flex items-center gap-2 rounded-lg border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <Play size={16} />
            启动服务
          </button>
        </div>

        {!hasExistingCredentials && (
          <p className="text-xs text-slate-500">
            首次启动前需要填写下方用户名和配对密码。
          </p>
        )}

        {connectionInfo && isConfigured && (
          <div className="space-y-2 rounded-xl border border-slate-200 bg-white p-4">
            <InfoRow label="Runner ID" value={connectionInfo.runner_id} />
            <InfoRow label="控制平面" value={connectionInfo.control_plane_url} />
            <InfoRow label="中继连接" value={connectionInfo.connected ? '已连接' : connectionInfo.running ? '连接中' : '未连接'} />
            <InfoRow label="自动启动" value={connectionInfo.auto_start ? '是' : '否'} />
          </div>
        )}
      </section>

      <section className="space-y-3">
        <div className="flex items-center gap-2">
          <ShieldCheck size={16} className="text-slate-600" />
          <h3 className="text-sm font-medium text-slate-700">账户与配对</h3>
        </div>

        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <User size={16} className="shrink-0 text-slate-400" />
            <input
              type="text"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              placeholder="用户名"
              className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
          </div>

          <div className="flex items-center gap-2">
            <div className="relative flex-1">
              <input
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                placeholder="密码（至少 4 位）"
                className="w-full rounded-lg border border-slate-300 px-3 py-2 pr-10 text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                minLength={4}
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-2 top-1/2 rounded p-1 text-slate-400 hover:text-slate-600"
                style={{ transform: 'translateY(-50%)' }}
                aria-label={showPassword ? '隐藏密码' : '显示密码'}
              >
                {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
            <button
              type="button"
              onClick={() => void handleSaveCredentials()}
              disabled={!username.trim() || password.length < 4 || savingCredentials}
              className="inline-flex shrink-0 items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {savingCredentials && <Loader2 size={16} className="animate-spin" />}
              {savingCredentials ? '保存中...' : savedCredentials ? '已保存' : '保存'}
            </button>
          </div>
        </div>

        {hasExistingCredentials && (
          <p className="text-xs text-amber-600">
            更改用户名或密码会生成新的身份标识，之前的远程会话将不会继续共享。
          </p>
        )}
      </section>

      {(message || error) && (
        <div
          className={`rounded-lg border px-3 py-2 text-xs ${
            error
              ? 'border-red-200 bg-red-50 text-red-700'
              : 'border-green-200 bg-green-50 text-green-700'
          }`}
        >
          {error || message}
        </div>
      )}
    </div>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="shrink-0 text-xs text-slate-500">{label}</span>
      <span className="min-w-0 truncate text-right font-mono text-xs text-slate-700" title={value}>
        {value || '--'}
      </span>
    </div>
  );
}
