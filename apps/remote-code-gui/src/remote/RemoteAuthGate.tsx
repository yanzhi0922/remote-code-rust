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
  onBootstrapClaim: () => void;
  onClearSavedToken: () => void;
  onManualTokenSave: () => void;
  onPairingAccept: () => void;
  pairingOfferId: string;
  pairingSecret: string;
  setBootstrapSecret: (value: string) => void;
  setDeviceName: (value: string) => void;
  setManualAccessToken: (value: string) => void;
  setPairingOfferId: (value: string) => void;
  setPairingSecret: (value: string) => void;
}

export function RemoteAuthGate({
  authErrorMessage,
  authLoading,
  bootstrapEnabled,
  copy,
  deviceName,
  health,
  manualAccessToken,
  onBootstrapClaim,
  onClearSavedToken,
  onManualTokenSave,
  onPairingAccept,
  pairingOfferId,
  pairingSecret,
  setBootstrapSecret,
  setDeviceName,
  setManualAccessToken,
  setPairingOfferId,
  setPairingSecret,
}: RemoteAuthGateProps) {
  return (
    <div className="mx-auto flex min-h-screen max-w-5xl items-center px-6 py-10">
      <div className="grid w-full gap-6 lg:grid-cols-[1.1fr_0.9fr]">
        <section className="rounded-[36px] border border-[#ddd2c1] bg-white px-7 py-7 shadow-[0_30px_70px_rgba(52,45,34,0.1)]">
          <div className="text-[11px] font-semibold uppercase tracking-[0.32em] text-slate-400">
            {copy.authGateEyebrow}
          </div>
          <div className="mt-3 text-3xl font-semibold text-slate-900">{copy.authGateTitle}</div>
          <div className="mt-4 max-w-2xl text-sm leading-7 text-slate-500">
            {copy.authGateDescription}
          </div>

          {authErrorMessage && (
            <div className="mt-5 flex items-start gap-3 rounded-3xl border border-[#f0d3c8] bg-[#fff2ed] px-4 py-4 text-sm leading-6 text-[#8d3f30]">
              <AlertTriangle size={18} className="mt-0.5 shrink-0" />
              <div>{authErrorMessage}</div>
            </div>
          )}

          <div className="mt-6 space-y-5">
            <label className="block">
              <div className="mb-2 text-xs font-semibold uppercase tracking-[0.2em] text-slate-400">
                {copy.deviceNameLabel}
              </div>
              <input
                value={deviceName}
                onChange={(event) => setDeviceName(event.target.value)}
                placeholder={copy.deviceNamePlaceholder}
                className="w-full rounded-2xl border border-[#dfd5c6] bg-[#fcfaf6] px-4 py-3 text-sm text-slate-800 outline-none transition-colors focus:border-[#a58a5e]"
              />
            </label>

            {bootstrapEnabled && (
              <div className="rounded-[28px] border border-[#e7dccd] bg-[#faf6ef] px-5 py-5">
                <div className="text-sm font-semibold text-slate-900">{copy.bootstrapTitle}</div>
                <div className="mt-2 text-sm leading-6 text-slate-500">{copy.bootstrapDescription}</div>
                <label className="mt-4 block">
                  <div className="mb-2 text-xs font-semibold uppercase tracking-[0.2em] text-slate-400">
                    {copy.bootstrapSecretLabel}
                  </div>
                  <input
                    type="password"
                    onChange={(event) => setBootstrapSecret(event.target.value)}
                    className="w-full rounded-2xl border border-[#dfd5c6] bg-white px-4 py-3 text-sm text-slate-800 outline-none transition-colors focus:border-[#a58a5e]"
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

            <div className="rounded-[28px] border border-[#e7dccd] bg-[#faf6ef] px-5 py-5">
              <div className="text-sm font-semibold text-slate-900">{copy.acceptPairingTitle}</div>
              <div className="mt-2 text-sm leading-6 text-slate-500">{copy.acceptPairingDescription}</div>
              <div className="mt-4 grid gap-3">
                <input
                  value={pairingOfferId}
                  onChange={(event) => setPairingOfferId(event.target.value)}
                  placeholder={copy.offerIdPlaceholder}
                  className="w-full rounded-2xl border border-[#dfd5c6] bg-white px-4 py-3 text-sm text-slate-800 outline-none transition-colors focus:border-[#a58a5e]"
                />
                <input
                  value={pairingSecret}
                  type="password"
                  onChange={(event) => setPairingSecret(event.target.value)}
                  placeholder={copy.pairingSecretPlaceholder}
                  className="w-full rounded-2xl border border-[#dfd5c6] bg-white px-4 py-3 text-sm text-slate-800 outline-none transition-colors focus:border-[#a58a5e]"
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

            <div className="rounded-[28px] border border-[#e7dccd] bg-[#faf6ef] px-5 py-5">
              <div className="text-sm font-semibold text-slate-900">{copy.existingTokenTitle}</div>
              <div className="mt-2 text-sm leading-6 text-slate-500">{copy.existingTokenDescription}</div>
              <textarea
                value={manualAccessToken}
                onChange={(event) => setManualAccessToken(event.target.value)}
                rows={3}
                placeholder="rcdt_..."
                className="mt-4 w-full rounded-2xl border border-[#dfd5c6] bg-white px-4 py-3 text-sm text-slate-800 outline-none transition-colors focus:border-[#a58a5e]"
              />
              <div className="mt-4 flex flex-wrap gap-3">
                <button
                  type="button"
                  onClick={onManualTokenSave}
                  className="inline-flex items-center gap-2 rounded-full border border-[#cfbfaa] bg-white px-4 py-2 text-sm font-medium text-slate-700 transition-colors hover:bg-[#fffaf2]"
                >
                  {copy.saveToken}
                </button>
                <button
                  type="button"
                  onClick={onClearSavedToken}
                  className="inline-flex items-center gap-2 rounded-full border border-[#eadccb] bg-transparent px-4 py-2 text-sm font-medium text-slate-500 transition-colors hover:bg-[#f4ecdf]"
                >
                  {copy.clearSavedToken}
                </button>
              </div>
            </div>
          </div>
        </section>

        <aside className="rounded-[36px] border border-[#ddd2c1] bg-[#f8f2e7] px-6 py-6 shadow-[0_24px_60px_rgba(52,45,34,0.08)]">
          <div className="text-[11px] font-semibold uppercase tracking-[0.32em] text-slate-400">
            {copy.controlPlaneEyebrow}
          </div>
          <div className="mt-3 text-xl font-semibold text-slate-900">{health.service}</div>
          <div className="mt-5 space-y-3 text-sm leading-6 text-slate-600">
            <div className="rounded-2xl bg-white/80 px-4 py-3">
              {copy.ownerClaimedLabel}: {health.owner_claimed ? copy.yes : copy.no}
            </div>
            <div className="rounded-2xl bg-white/80 px-4 py-3">
              {copy.trustedDevicesLabel}: {health.device_count}
            </div>
            <div className="rounded-2xl bg-white/80 px-4 py-3">
              {copy.availableRunnersLabel}: {health.available_runner_count}
            </div>
            <div className="rounded-2xl bg-white/80 px-4 py-3">
              {copy.activeSessionsLabel}: {health.session_count}
            </div>
            <div className="rounded-2xl bg-white/80 px-4 py-3">
              {copy.bootstrapConfiguredLabel}: {health.bootstrap_secret_configured ? copy.yes : copy.no}
            </div>
          </div>
          <div className="mt-6 rounded-3xl border border-dashed border-[#d4c4ac] bg-white/70 px-4 py-4 text-sm leading-6 text-slate-500">
            {copy.browserTokenNotice}
          </div>
        </aside>
      </div>
    </div>
  );
}
