import type { ErrorInfo, ReactNode } from 'react';
import { Component, useMemo, useState } from 'react';
import { getRemoteCopy, resolveRemoteLocale } from '../../remote/i18n';

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

export function AppErrorBoundary({ children }: ErrorBoundaryProps) {
  const copy = useMemo(() => getRemoteCopy(resolveRemoteLocale()), []);
  return <RootErrorBoundary copy={copy}>{children}</RootErrorBoundary>;
}

class RootErrorBoundary extends Component<
  ErrorBoundaryProps & { copy: ReturnType<typeof getRemoteCopy> },
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = {
    error: null,
  };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('remote-code-gui runtime error', error, errorInfo);
  }

  render() {
    if (this.state.error) {
      return <RuntimeErrorFallback copy={this.props.copy} error={this.state.error} />;
    }

    return this.props.children;
  }
}

function RuntimeErrorFallback({
  copy,
  error,
}: {
  copy: ReturnType<typeof getRemoteCopy>;
  error: Error;
}) {
  const [clearing, setClearing] = useState(false);

  const handleReload = () => {
    window.location.reload();
  };

  const handleClearCacheAndReload = async () => {
    setClearing(true);
    try {
      if ('serviceWorker' in navigator) {
        const registrations = await navigator.serviceWorker.getRegistrations();
        await Promise.all(registrations.map((registration) => registration.unregister()));
      }
      if ('caches' in window) {
        const keys = await window.caches.keys();
        await Promise.all(keys.map((key) => window.caches.delete(key)));
      }
    } finally {
      window.location.reload();
    }
  };

  return (
    <div className="flex min-h-screen items-center justify-center bg-[#f4efe4] px-6 py-10 text-slate-900">
      <div className="w-full max-w-lg rounded-[32px] border border-[#ddd2c1] bg-white px-6 py-6 shadow-[0_24px_60px_rgba(52,45,34,0.1)]">
        <div className="text-xl font-semibold text-slate-900">{copy.errorBoundaryTitle}</div>
        <div className="mt-3 text-sm leading-6 text-slate-600">{copy.errorBoundaryDescription}</div>
        <div className="mt-6 flex flex-wrap gap-3">
          <button
            type="button"
            onClick={handleReload}
            className="inline-flex items-center justify-center rounded-full bg-[#17181a] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[#2b2d30]"
          >
            {copy.errorBoundaryReload}
          </button>
          <button
            type="button"
            onClick={() => {
              void handleClearCacheAndReload();
            }}
            disabled={clearing}
            className="inline-flex items-center justify-center rounded-full border border-[#dccfc0] bg-[#faf6ef] px-4 py-2 text-sm font-medium text-slate-700 transition-colors hover:bg-[#f3ebdf] disabled:cursor-not-allowed disabled:opacity-60"
          >
            {clearing ? copy.errorBoundaryClearingCache : copy.errorBoundaryClearCache}
          </button>
        </div>
        <details className="mt-6 rounded-3xl border border-[#ece2d4] bg-[#faf7f1] px-4 py-4">
          <summary className="cursor-pointer text-sm font-medium text-slate-800">
            {copy.errorBoundaryDetails}
          </summary>
          <pre className="mt-3 overflow-x-auto whitespace-pre-wrap break-words text-xs leading-6 text-[#8d3f30]">
            {error.stack ?? error.message}
          </pre>
        </details>
      </div>
    </div>
  );
}
