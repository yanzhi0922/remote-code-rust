/**
 * MobileApp — Root component for the Capacitor mobile app.
 *
 * Phase 2: Integrates all native services:
 * - Biometric authentication on launch
 * - Push notifications for approvals
 * - Network status monitoring
 * - Deep link handling (QR pairing)
 * - Haptic feedback
 */

import { useState, useEffect, useCallback } from 'react';
import RemoteApp from '@remote/RemoteApp';
import { initMobileRuntime, persistRemotePairingContext } from './lib/runtime';
import { performBiometricCheck } from './native/biometric';
import {
  initPushNotifications,
  requestPushPermission,
  registerPushTokenWithControlPlane,
} from './native/pushNotifications';
import { initDeepLinks, parsePairingUrl } from './native/deepLink';
import { initNetworkMonitoring, getNetworkStatus, describeConnectionType, onNetworkChange } from './native/network';
import { hapticSuccess, hapticMedium, hapticWarning, hapticError } from './native/haptics';
import { resolveRemoteAccessToken, resolveRemoteBaseUrl } from './lib/runtime';

// ─── Splash Screen ──────────────────────────────────────────────────

function SplashScreen() {
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-[#f4efe4]">
      <div className="flex flex-col items-center gap-4">
        <div className="h-14 w-14 rounded-2xl bg-[#17181a] flex items-center justify-center shadow-lg">
          <span className="text-white text-xl font-bold">RC</span>
        </div>
        <div className="flex items-center gap-3 text-slate-500">
          <div className="h-5 w-5 rounded-full border-2 border-slate-300 border-t-slate-600 animate-spin" />
          <span className="text-sm font-medium">正在初始化...</span>
        </div>
      </div>
    </div>
  );
}

// ─── Error Screen ────────────────────────────────────────────────────

function ErrorScreen({ error, onRetry }: { error: string; onRetry: () => void }) {
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-[#f4efe4] px-6">
      <div className="max-w-sm text-center space-y-4">
        <div className="text-4xl">⚠️</div>
        <h1 className="text-lg font-bold text-slate-800">初始化失败</h1>
        <p className="text-sm text-slate-500 break-all">{error}</p>
        <button
          onClick={onRetry}
          className="px-4 py-2 bg-[#17181a] text-white rounded-lg text-sm font-medium hover:bg-[#2d2e30] transition-colors"
        >
          重试
        </button>
      </div>
    </div>
  );
}

// ─── Biometric Screen ────────────────────────────────────────────────

function BiometricScreen() {
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-[#f4efe4]">
      <div className="flex flex-col items-center gap-4">
        <div className="h-14 w-14 rounded-2xl bg-[#17181a] flex items-center justify-center shadow-lg">
          <span className="text-2xl">🔒</span>
        </div>
        <p className="text-sm text-slate-500 font-medium">请验证身份</p>
      </div>
    </div>
  );
}

// ─── Network Banner ──────────────────────────────────────────────────

function NetworkBanner({ online, connectionType }: { online: boolean; connectionType: string }) {
  if (online) return null;

  return (
    <div className="fixed top-0 left-0 right-0 z-50 bg-amber-500 text-white text-center py-1.5 text-xs font-medium shadow-md">
      网络已断开 — {describeConnectionType(connectionType)}
    </div>
  );
}

// ─── Main Component ──────────────────────────────────────────────────

type InitPhase = 'loading' | 'biometric' | 'ready' | 'error';

export default function MobileApp() {
  const [phase, setPhase] = useState<InitPhase>('loading');
  const [error, setError] = useState<string | null>(null);
  const [networkOnline, setNetworkOnline] = useState(true);
  const [connectionType, setConnectionType] = useState('unknown');

  const handleRetry = useCallback(() => {
    setError(null);
    setPhase('loading');
    void initialize();
  }, []);

  const initialize = useCallback(async () => {
    try {
      // Step 1: Initialize mobile runtime (load tokens from secure storage)
      await initMobileRuntime();

      // Step 2: Initialize network monitoring
      await initNetworkMonitoring();
      const netStatus = await getNetworkStatus();
      setNetworkOnline(netStatus.connected);
      setConnectionType(netStatus.connectionType);

      // Step 3: Initialize deep link handling
      initDeepLinks((url, _path, params) => {
        console.log('[DeepLink] Received:', url, params);
        // Handle pairing deep links
        const pairing = parsePairingUrl(url);
        if (pairing) {
          void persistRemotePairingContext(pairing.offerId, pairing.secret);
          window.dispatchEvent(new CustomEvent('deep-link-pairing', { detail: pairing }));
        }
      });

      // Step 4: Initialize push notifications
      await initPushNotifications({
        onApproval: (approvalId, sessionId) => {
          console.log('[Push] Approval notification:', approvalId, sessionId);
          hapticMedium();
          // Dispatch event for RemoteApp to handle
          window.dispatchEvent(
            new CustomEvent('push-approval', { detail: { approvalId, sessionId } }),
          );
        },
        onSessionUpdate: (sessionId) => {
          console.log('[Push] Session update:', sessionId);
          // Dispatch event for RemoteApp to handle
          window.dispatchEvent(
            new CustomEvent('push-session-update', { detail: { sessionId } }),
          );
        },
      });

      // Step 5: Request push permission (non-blocking)
      void requestPushPermission().then((granted) => {
        if (granted) {
          console.log('[Push] Permission granted');
        }
      });

      // Step 6: Register push token with Control Plane (if authenticated)
      const baseUrl = resolveRemoteBaseUrl();
      const accessToken = resolveRemoteAccessToken();
      if (baseUrl && accessToken) {
        void registerPushTokenWithControlPlane(baseUrl, accessToken);
      }

      // Step 7: Biometric check
      setPhase('biometric');
      const bioOk = await performBiometricCheck();
      if (!bioOk) {
        // Biometric failed, show error and exit
        hapticError();
        setError('身份验证失败');
        setPhase('error');
        return;
      }

      hapticSuccess();
      setPhase('ready');
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      setPhase('error');
    }
  }, []);

  // Network status listener
  useEffect(() => {
    const unsubscribe = onNetworkChange((connected, type) => {
      setNetworkOnline(connected);
      setConnectionType(type);
      if (!connected) {
        hapticWarning();
      }
    });

    return unsubscribe;
  }, []);

  // Initialize on mount
  useEffect(() => {
    void initialize();
  }, [initialize]);

  // ─── Render ──────────────────────────────────────────────────────

  if (phase === 'error' && error) {
    return <ErrorScreen error={error} onRetry={handleRetry} />;
  }

  if (phase === 'loading') {
    return <SplashScreen />;
  }

  if (phase === 'biometric') {
    return <BiometricScreen />;
  }

  return (
    <>
      <NetworkBanner online={networkOnline} connectionType={connectionType} />
      <RemoteApp />
    </>
  );
}
