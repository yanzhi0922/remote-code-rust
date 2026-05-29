import type { ErrorInfo, ReactNode } from 'react';
import { Component, useMemo, useState } from 'react';
import { logFrontendError } from '../../lib/frontendLogging';
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
    logFrontendError('react.error-boundary', error, errorInfo);
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
    <div role="alert" className="flex min-h-screen items-center justify-center bg-rc-bg-base px-6 py-10 text-rc-text-primary">
      <div className="w-full max-w-lg rounded-lg border border-rc-border-primary bg-rc-bg-surface px-6 py-6 shadow-lg">
        <div className="text-xl font-semibold text-rc-text-primary">{copy.errorBoundaryTitle}</div>
        <div className="mt-3 text-sm leading-6 text-rc-text-secondary">{copy.errorBoundaryDescription}</div>
        <div className="mt-6 flex flex-wrap gap-3">
          <button
            type="button"
            onClick={handleReload}
            className="inline-flex items-center justify-center rounded-md bg-rc-accent-primary px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-rc-accent-primary-hover"
          >
            {copy.errorBoundaryReload}
          </button>
          <button
            type="button"
            onClick={() => {
              void handleClearCacheAndReload();
            }}
            disabled={clearing}
            className="inline-flex items-center justify-center rounded-md border border-rc-border-primary bg-rc-bg-secondary px-4 py-2 text-sm font-medium text-rc-text-primary transition-colors hover:bg-rc-bg-hover disabled:cursor-not-allowed disabled:opacity-60"
          >
            {clearing ? copy.errorBoundaryClearingCache : copy.errorBoundaryClearCache}
          </button>
        </div>
        <details className="mt-6 rounded-md border border-rc-border-secondary bg-rc-bg-secondary px-4 py-4">
          <summary className="cursor-pointer text-sm font-medium text-rc-text-primary">
            {copy.errorBoundaryDetails}
          </summary>
          <pre className="mt-3 overflow-x-auto whitespace-pre-wrap break-words text-xs leading-6 text-rc-accent-error">
            {import.meta.env.PROD ? error.message : (error.stack ?? error.message)}
          </pre>
        </details>
      </div>
    </div>
  );
}
