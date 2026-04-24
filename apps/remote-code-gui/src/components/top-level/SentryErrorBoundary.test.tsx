import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SentryErrorBoundary } from './SentryErrorBoundary';

afterEach(() => {
  cleanup();
});

function ThrowError(): never {
  throw new Error('Test error');
}

describe('SentryErrorBoundary', () => {
  it('renders children when no error', () => {
    render(
      <SentryErrorBoundary>
        <div data-testid="child">OK</div>
      </SentryErrorBoundary>,
    );
    expect(screen.getByTestId('child')).toBeInTheDocument();
  });

  it('renders error UI when child throws', () => {
    // Suppress console.error for expected errors
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <SentryErrorBoundary>
        <ThrowError />
      </SentryErrorBoundary>,
    );
    expect(screen.getByTestId('sentry-error-boundary')).toBeInTheDocument();
    expect(screen.getByText('Test error')).toBeInTheDocument();
    spy.mockRestore();
  });

  it('renders custom fallback', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <SentryErrorBoundary fallback={<div data-testid="custom-fallback">Custom</div>}>
        <ThrowError />
      </SentryErrorBoundary>,
    );
    expect(screen.getByTestId('custom-fallback')).toBeInTheDocument();
    spy.mockRestore();
  });

  it('calls onError callback', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const onError = vi.fn();
    render(
      <SentryErrorBoundary onError={onError}>
        <ThrowError />
      </SentryErrorBoundary>,
    );
    expect(onError).toHaveBeenCalled();
    spy.mockRestore();
  });

  it('retries on button click', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    let shouldThrow = true;
    function ConditionalThrow() {
      if (shouldThrow) throw new Error('oops');
      return <div data-testid="recovered">Recovered</div>;
    }
    render(
      <SentryErrorBoundary>
        <ConditionalThrow />
      </SentryErrorBoundary>,
    );
    expect(screen.getByTestId('sentry-error-boundary')).toBeInTheDocument();
    shouldThrow = false;
    fireEvent.click(screen.getByTestId('sentry-error-retry'));
    expect(screen.getByTestId('recovered')).toBeInTheDocument();
    spy.mockRestore();
  });
});
