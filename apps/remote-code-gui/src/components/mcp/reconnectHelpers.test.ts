import { describe, expect, it } from 'vitest';
import {
  advanceReconnect,
  canRetry,
  createInitialReconnectState,
  formatReconnectProgress,
  getBackoffDelay,
  getReconnectStatusLabel,
  isRetryableError,
} from './reconnectHelpers';

describe('reconnectHelpers', () => {
  describe('getBackoffDelay', () => {
    it('returns base delay for attempt 0', () => {
      expect(getBackoffDelay(0)).toBe(1000);
    });

    it('doubles for each attempt', () => {
      expect(getBackoffDelay(1)).toBe(2000);
      expect(getBackoffDelay(2)).toBe(4000);
    });

    it('caps at 30000ms', () => {
      expect(getBackoffDelay(10)).toBe(30000);
    });

    it('respects custom base delay', () => {
      expect(getBackoffDelay(0, 500)).toBe(500);
      expect(getBackoffDelay(1, 500)).toBe(1000);
    });
  });

  describe('canRetry', () => {
    it('returns true when attempts remain', () => {
      expect(canRetry(0, 3)).toBe(true);
      expect(canRetry(2, 3)).toBe(true);
    });

    it('returns false when max reached', () => {
      expect(canRetry(3, 3)).toBe(false);
      expect(canRetry(5, 3)).toBe(false);
    });
  });

  describe('createInitialReconnectState', () => {
    it('creates initial state with defaults', () => {
      const state = createInitialReconnectState('test-server');
      expect(state.serverName).toBe('test-server');
      expect(state.attempt).toBe(0);
      expect(state.maxAttempts).toBe(3);
      expect(state.lastError).toBeNull();
      expect(state.timestamp).toBeGreaterThan(0);
    });

    it('respects custom maxAttempts', () => {
      const state = createInitialReconnectState('srv', 5);
      expect(state.maxAttempts).toBe(5);
    });
  });

  describe('advanceReconnect', () => {
    it('increments attempt', () => {
      const initial = createInitialReconnectState('srv');
      const next = advanceReconnect(initial);
      expect(next.attempt).toBe(1);
    });

    it('updates error when provided', () => {
      const initial = createInitialReconnectState('srv');
      const next = advanceReconnect(initial, 'timeout');
      expect(next.lastError).toBe('timeout');
    });

    it('preserves previous error when no new error', () => {
      const initial = createInitialReconnectState('srv');
      const step1 = advanceReconnect(initial, 'ECONNREFUSED');
      const step2 = advanceReconnect(step1);
      expect(step2.lastError).toBe('ECONNREFUSED');
    });
  });

  describe('getReconnectStatusLabel', () => {
    it('returns correct labels', () => {
      expect(getReconnectStatusLabel('idle')).toBe('空闲');
      expect(getReconnectStatusLabel('pending')).toBe('重连中');
      expect(getReconnectStatusLabel('success')).toBe('已连接');
      expect(getReconnectStatusLabel('failed')).toBe('重连失败');
    });
  });

  describe('formatReconnectProgress', () => {
    it('formats progress text', () => {
      const state = createInitialReconnectState('my-server');
      expect(formatReconnectProgress(state)).toBe('my-server (0/3)');
    });
  });

  describe('isRetryableError', () => {
    it('detects ECONNREFUSED', () => {
      expect(isRetryableError('Error: ECONNREFUSED')).toBe(true);
    });

    it('detects timeout', () => {
      expect(isRetryableError('Connection timeout')).toBe(true);
    });

    it('detects network errors', () => {
      expect(isRetryableError('network error')).toBe(true);
    });

    it('detects fetch failed', () => {
      expect(isRetryableError('fetch failed')).toBe(true);
    });

    it('returns false for non-retryable errors', () => {
      expect(isRetryableError('Invalid API key')).toBe(false);
      expect(isRetryableError('Permission denied')).toBe(false);
    });
  });
});
