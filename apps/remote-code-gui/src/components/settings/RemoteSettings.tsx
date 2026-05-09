import { useCallback, useEffect, useState } from 'react';
import { Eye, EyeOff, Loader2, Wifi, WifiOff, Smartphone, User } from 'lucide-react';
import * as tauri from '../../lib/tauri';

export function RemoteSettings() {
  const [status, setStatus] = useState<'loading' | 'enabled' | 'disabled'>('loading');
  const [connectionInfo, setConnectionInfo] = useState<Record<string, string> | null>(null);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hasExistingCredentials, setHasExistingCredentials] = useState(false);

  useEffect(() => {
    tauri.remoteHasPassword()
      .then((has) => setHasExistingCredentials(has))
      .catch(() => setHasExistingCredentials(false));
  }, []);

  useEffect(() => {
    tauri.remoteGetStatus()
      .then((s) => setStatus(s === 'enabled' ? 'enabled' : 'disabled'))
      .catch(() => setStatus('disabled'));

    tauri.remoteGetConnectionInfo()
      .then(setConnectionInfo)
      .catch(() => setConnectionInfo(null));

    tauri.remoteGetUsername()
      .then((u) => { if (u) setUsername(u); })
      .catch(() => {});
  }, []);

  const handleSaveCredentials = useCallback(async () => {
    if (!username.trim() || password.length < 4) return;
    setSaving(true);
    setSaved(false);
    setError(null);
    try {
      await tauri.remoteSetCredentials(username, password);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(e instanceof Error ? e.message : '保存失败');
    } finally {
      setSaving(false);
    }
  }, [username, password]);

  const isEnabled = status === 'enabled';

  return (
    <div className="space-y-6">
      {/* Status */}
      <div className="rounded-xl border border-slate-200 bg-slate-50 p-4">
        <div className="flex items-center gap-3">
          {status === 'loading' ? (
            <Loader2 size={20} className="animate-spin text-slate-400" />
          ) : isEnabled ? (
            <Wifi size={20} className="text-green-600" />
          ) : (
            <WifiOff size={20} className="text-slate-400" />
          )}
          <div className="flex-1">
            <div className="text-sm font-medium text-slate-800">
              {status === 'loading'
                ? '检测中...'
                : isEnabled
                  ? '远程控制已启用'
                  : '远程控制未启用'}
            </div>
            <p className="text-xs text-slate-500">
              {isEnabled
                ? '手机 App 可以远程连接到此电脑'
                : '设置控制平面 URL 以启用远程控制'}
            </p>
          </div>
          <span
            className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${
              isEnabled
                ? 'bg-green-100 text-green-700'
                : 'bg-slate-100 text-slate-500'
            }`}
          >
            {status === 'loading' ? '--' : isEnabled ? 'ON' : 'OFF'}
          </span>
        </div>
      </div>

      {/* Connection Info */}
      {connectionInfo && isEnabled && (
        <div className="space-y-3">
          <h3 className="text-sm font-medium text-slate-700">连接信息</h3>
          <div className="space-y-2 rounded-xl border border-slate-200 bg-white p-4">
            <InfoRow label="Runner ID" value={connectionInfo.runner_id} />
            <InfoRow label="控制平面" value={connectionInfo.control_plane_url} />
          </div>
        </div>
      )}

      {/* User Identity & Password */}
      {isEnabled && (
        <div className="space-y-3">
          <div className="flex items-center gap-2">
            <Smartphone size={16} className="text-slate-600" />
            <h3 className="text-sm font-medium text-slate-700">账户与配对</h3>
          </div>
          <p className="text-xs text-slate-500">
            设置用户名和密码来隔离通信。多用户共享同一个控制平面时，相同用户名+密码的设备互相可见。
            服务器不存储密码，仅用于本地派生身份标识。
          </p>

          <div className="space-y-2">
            {/* Username */}
            <div className="flex items-center gap-2">
              <User size={16} className="text-slate-400 shrink-0" />
              <input
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="用户名"
                className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
            </div>

            {/* Password */}
            <div className="flex items-center gap-2">
              <div className="relative flex-1">
                <input
                  type={showPassword ? 'text' : 'password'}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="密码（至少 4 位）"
                  className="w-full rounded-lg border border-slate-300 px-3 py-2 pr-10 text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                  minLength={4}
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-1 text-slate-400 hover:text-slate-600"
                >
                  {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                </button>
              </div>
              <button
                type="button"
                onClick={() => void handleSaveCredentials()}
                disabled={!username.trim() || password.length < 4 || saving}
                className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
              >
                {saving ? '保存中...' : saved ? '已保存 ✓' : '保存'}
              </button>
            </div>
          </div>
          {error && <p className="text-xs text-red-600">{error}</p>}
          {hasExistingCredentials && (
            <p className="text-xs text-amber-600">
              ⚠ 更改用户名或密码会生成新的身份标识，之前的会话和数据将无法访问。
            </p>
          )}
        </div>
      )}

      {/* How it works */}
      {isEnabled && (
        <div className="rounded-xl border border-blue-200 bg-blue-50 p-4">
          <h4 className="text-sm font-medium text-blue-800">使用说明</h4>
          <ol className="mt-2 space-y-1 text-xs text-blue-700">
            <li>1. 在电脑和手机上设置相同的用户名和密码</li>
            <li>2. 在手机 App 中输入控制平面地址</li>
            <li>3. 连接成功后即可远程发送命令</li>
            <li>4. 不同用户名/密码的用户之间数据完全隔离</li>
          </ol>
        </div>
      )}
    </div>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs text-slate-500">{label}</span>
      <span className="max-w-[200px] truncate text-xs font-mono text-slate-700" title={value}>
        {value || '--'}
      </span>
    </div>
  );
}
