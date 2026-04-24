export interface SecurityCheckResult {
  secure: boolean;
  warnings: string[];
  errors: string[];
}

export function checkManagedSettingsSecurity(settings: Record<string, unknown>): SecurityCheckResult {
  const warnings: string[] = [];
  const errors: string[] = [];

  if (settings.permissions && typeof settings.permissions === 'object') {
    const perms = settings.permissions as Record<string, unknown>;
    if (perms.defaultMode === 'bypass') {
      errors.push('不允许设置绕过权限模式');
    }
    if (perms.allowAll === true) {
      errors.push('不允许允许所有工具');
    }
  }

  if (settings.apiKey && typeof settings.apiKey === 'string') {
    warnings.push('API密钥应通过安全存储管理，不应在设置中明文存储');
  }

  if (settings.shell && typeof settings.shell === 'object') {
    const shell = settings.shell as Record<string, unknown>;
    if (shell.unrestricted === true) {
      warnings.push('不受限制的Shell访问可能存在安全风险');
    }
  }

  return {
    secure: errors.length === 0,
    warnings,
    errors,
  };
}

export function sanitizeSettingValue(key: string, value: unknown): unknown {
  if (key.toLowerCase().includes('password') || key.toLowerCase().includes('secret')) {
    return '********';
  }
  if (typeof value === 'string') {
    return value.replace(/<script[^>]*>.*?<\/script>/gi, '').trim();
  }
  return value;
}
