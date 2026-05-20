import { AlertTriangle, LoaderCircle, Shield, Wifi } from 'lucide-react';
import type { RemoteCopy } from './i18n';
import type { RemoteControlPlaneHealth } from './types';

interface RemoteAuthGateProps {
  authErrorMessage: string | null;
  authLoading: boolean;
  bootstrapEnabled: boolean;
  copy: RemoteCopy;
  deviceName: string;
  health: RemoteControlPlaneHealth;
  manualAccessToken: string;
  pairingOfferId: string;
  pairingSecret: string;
  username: string;
  password: string;
  onBootstrapClaim: () => void;
  onClearSavedToken: () => void;
  onManualTokenSave: () => void;
  onPairingAccept: () => void;
  onUserSignIn: () => void;
  setBootstrapSecret: (value: string) => void;
  setDeviceName: (value: string) => void;
  setManualAccessToken: (value: string) => void;
  setPairingOfferId: (value: string) => void;
  setPairingSecret: (value: string) => void;
  setUsername: (value: string) => void;
  setPassword: (value: string) => void;
}

export function RemoteAuthGate({
  authErrorMessage,
  authLoading,
  bootstrapEnabled,
  copy,
  deviceName,
  health,
  manualAccessToken,
  pairingOfferId,
  pairingSecret,
  username,
  password,
  onBootstrapClaim,
  onClearSavedToken,
  onManualTokenSave,
  onPairingAccept,
  onUserSignIn,
  setBootstrapSecret,
  setDeviceName,
  setManualAccessToken,
  setPairingOfferId,
  setPairingSecret,
  setUsername,
  setPassword,
}: RemoteAuthGateProps) {
  return (
    <div className="mx-auto flex min-h-screen max-w-5xl items-center px-6 py-10">
      <div className="grid w-full gap-6 lg:grid-cols-[1.1fr_0.9fr]">
        <section className="rounded-3xl border border-rc-border-primary bg-rc-bg-surface px-7 py-7 shadow-[0_30px_70px_rgba(52,45,34,0.1)]">
          <div className="text-[11px] font-semibold uppercase tracking-[0.32em] text-rc-text-tertiary">
            {copy.authGateEyebrow}
          </div>
          <div className="mt-3 text-3xl font-semibold text-rc-text-primary">{copy.authGateTitle}</div>
          <div className="mt-4 max-w-2xl text-sm leading-7 text-rc-text-tertiary">
            {copy.authGateDescription}
          </div>

          {authErrorMessage && (
            <div role="alert" className="mt-5 flex items-start gap-3 rounded-3xl border border-[#f0d3c8] bg-[#fff2ed] px-4 py-4 text-sm leading-6 text-[#8d3f30]">
              <AlertTriangle size={18} className="mt-0.5 shrink-0" />
              <div>{authErrorMessage}</div>
            </div>
          )}

          <div className="mt-6 space-y-5">
            <label className="block">
              <div className="mb-2 text-xs font-semibold uppercase tracking-[0.2em] text-rc-text-tertiary">
                {copy.deviceNameLabel}
              </div>
              <input
                value={deviceName}
                onChange={(event) => setDeviceName(event.target.value)}
                placeholder={copy.deviceNamePlaceholder}
                className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-secondary px-4 py-3 text-sm text-rc-text-primary outline-none transition-colors focus:border-[#a58a5e]"
              />
            </label>

            {/* ── Multi-user sign-in ── */}
            <div className="rounded-3xl border border-rc-border-primary bg-rc-bg-hover px-5 py-5">
              <div className="text-sm font-semibold text-rc-text-primary">{copy.multiUserTitle}</div>
              <div className="mt-2 text-sm leading-6 text-rc-text-tertiary">{copy.multiUserDescription}</div>
              <div className="mt-4 grid gap-3">
                <input
                  value={username}
                  onChange={(event) => setUsername(event.target.value)}
                  placeholder={copy.usernamePlaceholder}
                  autoComplete="username"
                  className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary outline-none transition-colors focus:border-[#a58a5e]"
                />
                <input
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  type="password"
                  placeholder={copy.passwordPlaceholder}
                  autoComplete="current-password"
                  className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary outline-none transition-colors focus:border-[#a58a5e]"
                />
              </div>
              <button
                type="button"
                onClick={onUserSignIn}
                disabled={authLoading || !username.trim() || !password.trim()}
                className="mt-4 inline-flex items-center gap-2 rounded-full bg-[#1d6b45] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[#145033] disabled:cursor-not-allowed disabled:opacity-60"
              >
                {authLoading ? <LoaderCircle size={15} className="animate-spin" /> : <Shield size={15} />}
                {copy.signInAction}
              </button>
            </div>

            {bootstrapEnabled && (
              <div className="rounded-3xl border border-rc-border-primary bg-rc-bg-hover px-5 py-5">
                <div className="text-sm font-semibold text-rc-text-primary">{copy.bootstrapTitle}</div>
                <div className="mt-2 text-sm leading-6 text-rc-text-tertiary">{copy.bootstrapDescription}</div>
                <label className="mt-4 block">
                  <div className="mb-2 text-xs font-semibold uppercase tracking-[0.2em] text-rc-text-tertiary">
                    {copy.bootstrapSecretLabel}
                  </div>
                  <input
                    type="password"
                    onChange={(event) => setBootstrapSecret(event.target.value)}
                    className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary outline-none transition-colors focus:border-[#a58a5e]"
                  />
                </label>
                <button
                  type="button"
                  onClick={onBootstrapClaim}
                  disabled={authLoading}
                  className="mt-4 inline-flex items-center gap-2 rounded-full bg-[#1d6b45] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[#145033] disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {authLoading ? <LoaderCircle size={15} className="animate-spin" /> : <Shield size={15} />}
                  {copy.claimOwnerDevice}
                </button>
              </div>
            )}

            <div className="rounded-3xl border border-rc-border-primary bg-rc-bg-hover px-5 py-5">
              <div className="text-sm font-semibold text-rc-text-primary">{copy.acceptPairingTitle}</div>
              <div className="mt-2 text-sm leading-6 text-rc-text-tertiary">{copy.acceptPairingDescription}</div>
              <div className="mt-4 grid gap-3">
                <input
                  value={pairingOfferId}
                  onChange={(event) => setPairingOfferId(event.target.value)}
                  placeholder={copy.offerIdPlaceholder}
                  className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary outline-none transition-colors focus:border-[#a58a5e]"
                />
                <input
                  value={pairingSecret}
                  type="password"
                  onChange={(event) => setPairingSecret(event.target.value)}
                  placeholder={copy.pairingSecretPlaceholder}
                  className="w-full rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary outline-none transition-colors focus:border-[#a58a5e]"
                />
              </div>
              <button
                type="button"
                onClick={onPairingAccept}
                disabled={authLoading}
                className="mt-4 inline-flex items-center gap-2 rounded-full bg-[#174e8c] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[#123b6b] disabled:cursor-not-allowed disabled:opacity-60"
              >
                {authLoading ? <LoaderCircle size={15} className="animate-spin" /> : <Wifi size={15} />}
                {copy.acceptPairingAction}
              </button>
            </div>

            <div className="rounded-3xl border border-rc-border-primary bg-rc-bg-hover px-5 py-5">
              <div className="text-sm font-semibold text-rc-text-primary">{copy.existingTokenTitle}</div>
              <div className="mt-2 text-sm leading-6 text-rc-text-tertiary">{copy.existingTokenDescription}</div>
              <textarea
                value={manualAccessToken}
                onChange={(event) => setManualAccessToken(event.target.value)}
                rows={3}
                placeholder="rcdt_..."
                className="mt-4 w-full rounded-2xl border border-rc-border-primary bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary outline-none transition-colors focus:border-[#a58a5e]"
              />
              <div className="mt-4 flex flex-wrap gap-3">
                <button
                  type="button"
                  onClick={onManualTokenSave}
                  className="inline-flex items-center gap-2 rounded-full border border-rc-border-primary bg-rc-bg-surface px-4 py-2 text-sm font-medium text-rc-text-secondary transition-colors hover:bg-rc-bg-hover"
                >
                  {copy.saveToken}
                </button>
                <button
                  type="button"
                  onClick={onClearSavedToken}
                  className="inline-flex items-center gap-2 rounded-full border border-rc-border-primary bg-transparent px-4 py-2 text-sm font-medium text-rc-text-tertiary transition-colors hover:bg-rc-bg-active"
                >
                  {copy.clearSavedToken}
                </button>
              </div>
            </div>
          </div>
        </section>

        <aside className="rounded-3xl border border-rc-border-primary bg-rc-bg-secondary px-6 py-6 shadow-[0_24px_60px_rgba(52,45,34,0.08)]">
          <div className="text-[11px] font-semibold uppercase tracking-[0.32em] text-rc-text-tertiary">
            {copy.controlPlaneEyebrow}
          </div>
          <div className="mt-3 text-xl font-semibold text-rc-text-primary">{health.service}</div>
          <div className="mt-5 space-y-3 text-sm leading-6 text-rc-text-secondary">
            <div className="rounded-2xl bg-rc-bg-surface/80 px-4 py-3">
              {copy.ownerClaimedLabel}: {health.owner_claimed ? copy.yes : copy.no}
            </div>
            <div className="rounded-2xl bg-rc-bg-surface/80 px-4 py-3">
              {copy.trustedDevicesLabel}: {health.device_count}
            </div>
            <div className="rounded-2xl bg-rc-bg-surface/80 px-4 py-3">
              {copy.availableRunnersLabel}: {health.available_runner_count}
            </div>
            <div className="rounded-2xl bg-rc-bg-surface/80 px-4 py-3">
              {copy.activeSessionsLabel}: {health.session_count}
            </div>
            <div className="rounded-2xl bg-rc-bg-surface/80 px-4 py-3">
              {copy.bootstrapConfiguredLabel}: {health.bootstrap_secret_configured ? copy.yes : copy.no}
            </div>
          </div>
          <div className="mt-6 rounded-3xl border border-dashed border-rc-border-primary bg-rc-bg-surface/70 px-4 py-4 text-sm leading-6 text-rc-text-tertiary">
            {copy.browserTokenNotice}
          </div>
        </aside>
      </div>
    </div>
  );
}