import { describe, expect, it } from 'vitest';
import { checkManagedSettingsSecurity, sanitizeSettingValue } from './utils';

describe('checkManagedSettingsSecurity', () => {
  it('returns secure for empty settings', () => {
    const result = checkManagedSettingsSecurity({});
    expect(result.secure).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it('detects bypass permission mode', () => {
    const result = checkManagedSettingsSecurity({
      permissions: { defaultMode: 'bypass' },
    });
    expect(result.secure).toBe(false);
    expect(result.errors).toContain('不允许设置绕过权限模式');
  });

  it('detects allowAll', () => {
    const result = checkManagedSettingsSecurity({
      permissions: { allowAll: true },
    });
    expect(result.secure).toBe(false);
    expect(result.errors).toContain('不允许允许所有工具');
  });

  it('warns about API keys in settings', () => {
    const result = checkManagedSettingsSecurity({ apiKey: 'sk-test' });
    expect(result.warnings.length).toBeGreaterThan(0);
  });

  it('warns about unrestricted shell', () => {
    const result = checkManagedSettingsSecurity({ shell: { unrestricted: true } });
    expect(result.warnings.length).toBeGreaterThan(0);
  });
});

describe('sanitizeSettingValue', () => {
  it('trims string values', () => {
    expect(sanitizeSettingValue('name', '  hello  ')).toBe('hello');
  });

  it('removes script tags', () => {
    expect(sanitizeSettingValue('html', '<script>alert(1)</script>hello')).toBe('hello');
  });

  it('masks password fields', () => {
    expect(sanitizeSettingValue('db_password', 'secret123')).toBe('********');
  });

  it('passes through non-sensitive values', () => {
    expect(sanitizeSettingValue('count', 42)).toBe(42);
  });
});
