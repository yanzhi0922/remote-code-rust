import { describe, expect, it } from 'vitest';
import { serializeFrontendError } from './frontendLogging';

describe('frontend logging serialization', () => {
  it('serializes Error objects with bounded stack and details fields', () => {
    const error = new Error('boom');
    error.stack = `Error: boom\n${'x'.repeat(10_000)}`;

    const event = serializeFrontendError('boundary', error, 'd'.repeat(10_000));

    expect(event.level).toBe('error');
    expect(event.source).toBe('boundary');
    expect(event.message).toBe('boom');
    expect(event.stack?.length ?? 0).toBeLessThanOrEqual(4_160);
    expect(event.details?.length ?? 0).toBeLessThanOrEqual(4_160);
  });

  it('redacts obvious secret fields before shipping browser diagnostics to Rust', () => {
    const event = serializeFrontendError('global', {
      message: 'request failed',
      api_key: 'secret-token',
      Authorization: 'Bearer super-secret',
      nested: { password: 'hunter2' },
    });

    const payload = JSON.stringify(event);
    expect(payload).toContain('[redacted]');
    expect(payload).not.toContain('secret-token');
    expect(payload).not.toContain('super-secret');
    expect(payload).not.toContain('hunter2');
  });
});
