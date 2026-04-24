import { Component, type ErrorInfo, type ReactNode } from 'react';
import { AlertTriangle, RotateCcw } from 'lucide-react';

export interface SentryErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
  onError?: (error: Error, errorInfo: ErrorInfo) => void;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

export class SentryErrorBoundary extends Component<SentryErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: SentryErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    this.props.onError?.(error, errorInfo);
  }

  handleRetry = (): void => {
    this.setState({ hasError: false, error: null });
  };

  render(): ReactNode {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }
      return (
        <div data-testid="sentry-error-boundary" className="flex flex-col items-center justify-center gap-4 p-8">
          <AlertTriangle className="h-8 w-8 text-red-500" />
          <h2 className="text-lg font-semibold text-slate-800">出了点问题</h2>
          <p className="text-sm text-slate-500">
            {this.state.error?.message ?? '发生了未知错误'}
          </p>
          <button
            type="button"
            data-testid="sentry-error-retry"
            className="inline-flex items-center gap-1.5 rounded bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700"
            onClick={this.handleRetry}
          >
            <RotateCcw className="h-4 w-4" />
            重试
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
