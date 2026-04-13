import { useEffect } from 'react';
import { Layout } from './components/layout/Layout';
import { PermissionModal } from './components/layout/PermissionModal';
import { ChatArea } from './components/chat/ChatArea';
import { ChatInput } from './components/chat/ChatInput';
import { shouldUseRemoteMode } from './lib/runtime';
import RemoteApp from './remote/RemoteApp';
import { useAppStore } from './stores/useAppStore';

function App() {
  if (shouldUseRemoteMode()) {
    return <RemoteApp />;
  }

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

export default App;
