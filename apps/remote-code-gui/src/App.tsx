import { useState, useEffect, useCallback } from 'react';
import { Layout } from './components/layout/Layout';
import { PermissionModal } from './components/layout/PermissionModal';
import { ChatArea } from './components/chat/ChatArea';
import { ChatInput } from './components/chat/ChatInput';
import { shouldUseRemoteMode } from './lib/runtime';
import { isMobileSync, isTouchDevice } from './lib/mobile';
import {
  performBiometricCheck,
  initNetworkMonitoring,
  getNetworkStatus,
  onNetworkChange,
  describeConnectionType,
  hapticSuccess,
  hapticError,
  hapticWarning,
} from './lib/mobile';
import RemoteApp from './remote/RemoteApp';
import { useAppStore } from './stores/useAppStore';

type MobileInitPhase = 'loading' | 'biometric' | 'ready' | 'error';

function MobileInitScreen() {
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

function MobileBiometricScreen() {
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

function MobileErrorScreen({ error, onRetry }: { error: string; onRetry: () => void }) {
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

function MobileNetworkBanner({ online, connectionType }: { online: boolean; connectionType: string }) {
  if (online) return null;
  return (
    <div className="fixed top-0 left-0 right-0 z-50 bg-amber-500 text-white text-center py-1.5 text-xs font-medium shadow-md">
      网络已断开 — {describeConnectionType(connectionType)}
    </div>
  );
}

function MobileGate({ children }: { children: React.ReactNode }) {
  const [phase, setPhase] = useState<MobileInitPhase>('loading');
  const [error, setError] = useState<string | null>(null);
  const [networkOnline, setNetworkOnline] = useState(true);
  const [connectionType, setConnectionType] = useState('unknown');

  const initialize = useCallback(async () => {
    try {
      initNetworkMonitoring();
      const netStatus = getNetworkStatus();
      setNetworkOnline(netStatus.connected);
      setConnectionType(netStatus.connectionType);

      setPhase('biometric');
      const bioOk = await performBiometricCheck();
      if (!bioOk) {
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

  useEffect(() => {
    onNetworkChange((connected, type) => {
      setNetworkOnline(connected);
      setConnectionType(type);
      if (!connected) hapticWarning();
    });
  }, []);

  useEffect(() => {
    void initialize();
  }, [initialize]);

  if (phase === 'error' && error) {
    return <MobileErrorScreen error={error} onRetry={() => { setError(null); setPhase('loading'); void initialize(); }} />;
  }
  if (phase === 'loading') return <MobileInitScreen />;
  if (phase === 'biometric') return <MobileBiometricScreen />;

  return (
    <>
      <MobileNetworkBanner online={networkOnline} connectionType={connectionType} />
      {children}
    </>
  );
}

function LocalApp() {
  const initialised = useAppStore((s) => s.initialised);
  const initError = useAppStore((s) => s.initError);
  const init = useAppStore((s) => s.init);

  useEffect(() => {
    init();
  }, [init]);

  if (initError) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-[#f8f7f4]">
        <div className="max-w-md text-center space-y-4">
          <div className="text-4xl">⚠️</div>
          <h1 className="text-lg font-bold text-slate-800">初始化失败</h1>
          <p className="text-sm text-slate-500 break-all">{initError}</p>
          <button
            onClick={() => init()}
            className="px-4 py-2 bg-[#17181a] text-white rounded-lg text-sm font-medium hover:bg-[#2d2e30] transition-colors"
          >
            重试
          </button>
        </div>
      </div>
    );
  }

  if (!initialised) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-[#f8f7f4]">
        <div className="flex items-center gap-3 text-slate-500">
          <div className="w-5 h-5 border-2 border-slate-300 border-t-slate-600 rounded-full animate-spin" />
          <span className="text-sm font-medium">正在初始化...</span>
        </div>
      </div>
    );
  }

  return (
    <>
      <Layout>
        <div className="flex h-full flex-col bg-[#f7f4ed]">
          <ChatArea />
          <ChatInput />
        </div>
      </Layout>
      <PermissionModal />
    </>
  );
}

function App() {
  const mobile = isMobileSync() || isTouchDevice();

  if (shouldUseRemoteMode()) {
    if (mobile) {
      return (
        <MobileGate>
          <RemoteApp />
        </MobileGate>
      );
    }
    return <RemoteApp />;
  }

  if (mobile) {
    return (
      <MobileGate>
        <LocalApp />
      </MobileGate>
    );
  }

  return <LocalApp />;
}

export default App;
