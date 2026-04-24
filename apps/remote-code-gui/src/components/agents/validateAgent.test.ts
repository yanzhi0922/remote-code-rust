import { describe, expect, it } from 'vitest';
import { validateAgent } from './validateAgent';

describe('validateAgent', () => {
  it('returns valid for complete agent', () => {
    const result = validateAgent({ name: 'Test Agent', prompt: 'Do stuff', tools: ['bash'] });
    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it('returns error for missing name', () => {
    const result = validateAgent({});
    expect(result.valid).toBe(false);
    expect(result.errors).toContain('名称不能为空');
  });

  it('returns error for too long name', () => {
    const result = validateAgent({ name: 'a'.repeat(101) });
    expect(result.valid).toBe(false);
    expect(result.errors).toContain('名称不能超过100个字符');
  });

  it('warns about missing prompt', () => {
    const result = validateAgent({ name: 'Test' });
    expect(result.warnings).toContain('建议添加提示词');
  });

  it('warns about empty tools', () => {
    const result = validateAgent({ name: 'Test', prompt: 'Do stuff', tools: [] });
    expect(result.warnings).toContain('未指定工具，Agent将无法执行操作');
  });
});
