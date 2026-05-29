import { hasTauriRuntime } from './runtime';
import { recordFrontendLog } from './tauri';
import type { FrontendLogEvent, FrontendLogLevel } from './types';

const MAX_FIELD_CHARS = 4096;
const MAX_REDACTION_DEPTH = 8;
const SENSITIVE_KEYS = [
  'api_key',
  'apikey',
  'authorization',
  'password',
  'secret',
  'token',
  'refresh_token',
  'access_token',
];

export function serializeFrontendError(
  source: string,
  error: unknown,
  details?: unknown,
): FrontendLogEvent {
  const safeError = redactSensitiveValue(error);
  const safeDetails = details === undefined ? undefined : redactSensitiveValue(details);
  const derivedDetails =
    safeDetails ??
    (error instanceof Error || typeof error === 'string' ? undefined : safeError);

  return {
    level: 'error',
    source: boundedRequiredString(source, 'frontend'),
    message: boundedRequiredString(resolveErrorMessage(safeError), 'Unknown frontend error'),
    details: derivedDetails === undefined ? null : boundedString(stringifyDiagnosticValue(derivedDetails)),
    stack: boundedString(resolveErrorStack(safeError)),
    url: boundedString(window.location.href),
    userAgent: boundedString(window.navigator.userAgent),
  };
}

export function logFrontendError(source: string, error: unknown, details?: unknown): void {
  logFrontendEvent(serializeFrontendError(source, error, details));
}

export function logFrontendEvent(event: FrontendLogEvent): void {
  if (!hasTauriRuntime()) {
    return;
  }

  const normalized: FrontendLogEvent = {
    level: normalizeLevel(event.level),
    source: boundedRequiredString(event.source, 'frontend'),
    message: boundedRequiredString(event.message, 'frontend log event'),
    details: event.details == null ? null : boundedString(event.details),
    stack: event.stack == null ? null : boundedString(event.stack),
    url: event.url == null ? null : boundedString(event.url),
    line: event.line ?? null,
    column: event.column ?? null,
    userAgent: event.userAgent == null ? null : boundedString(event.userAgent),
  };

  void recordFrontendLog(normalized).catch(() => {
    // Logging must never create a secondary UI failure.
  });
}

function normalizeLevel(level: FrontendLogLevel): FrontendLogLevel {
  return ['debug', 'info', 'warn', 'error'].includes(level) ? level : 'info';
}

function resolveErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message || error.name || 'Unknown frontend error';
  }
  if (isRecord(error) && typeof error.message === 'string') {
    return error.message;
  }
  if (typeof error === 'string') {
    return error;
  }
  return stringifyDiagnosticValue(error);
}

function resolveErrorStack(error: unknown): string | null {
  if (error instanceof Error) {
    return error.stack ?? null;
  }
  if (isRecord(error) && typeof error.stack === 'string') {
    return error.stack;
  }
  return null;
}

function stringifyDiagnosticValue(value: unknown): string {
  if (value == null) {
    return '';
  }
  if (typeof value === 'string') {
    return value;
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function redactSensitiveValue(value: unknown, depth = 0): unknown {
  if (depth > MAX_REDACTION_DEPTH) {
    return '[truncated-depth]';
  }
  if (Array.isArray(value)) {
    return value.map((item) => redactSensitiveValue(item, depth + 1));
  }
  if (value instanceof Error) {
    const copy: Record<string, unknown> = {
      name: value.name,
      message: value.message,
      stack: value.stack,
    };
    for (const key of Object.keys(value)) {
      copy[key] = redactSensitiveValue((value as unknown as Record<string, unknown>)[key], depth + 1);
    }
    return copy;
  }
  if (isRecord(value)) {
    const copy: Record<string, unknown> = {};
    for (const [key, nested] of Object.entries(value)) {
      copy[key] = isSensitiveKey(key) ? '[redacted]' : redactSensitiveValue(nested, depth + 1);
    }
    return copy;
  }
  return value;
}

function isSensitiveKey(key: string): boolean {
  const lower = key.toLowerCase();
  return SENSITIVE_KEYS.some((marker) => lower.includes(marker));
}

function boundedString(value: string | null | undefined): string | null {
  if (value == null) {
    return null;
  }
  if (value.length <= MAX_FIELD_CHARS) {
    return value;
  }
  return `${value.slice(0, MAX_FIELD_CHARS)}\n[truncated]`;
}

function boundedRequiredString(value: string | null | undefined, fallback: string): string {
  return boundedString(value) ?? fallback;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}
