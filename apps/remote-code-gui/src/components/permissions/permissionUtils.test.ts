import { describe, expect, it } from 'vitest';
import { getToolCategory, getCategoryIcon, extractFilePath, formatPermissionRule, isDangerousPermission } from './permissionUtils';
import type { PermissionRequestInfo } from '../../lib/types';

describe('permissionUtils', () => {
  describe('getToolCategory', () => {
    it('detects shell tools', () => {
      expect(getToolCategory('Bash')).toBe('shell');
    });

    it('detects filesystem tools', () => {
      expect(getToolCategory('FileEdit')).toBe('filesystem');
    });

    it('detects network tools', () => {
      expect(getToolCategory('WebFetch')).toBe('network');
    });

    it('returns other for unknown', () => {
      expect(getToolCategory('Custom')).toBe('other');
    });
  });

  describe('getCategoryIcon', () => {
    it('returns icon name', () => {
      expect(getCategoryIcon('shell')).toBe('Terminal');
      expect(getCategoryIcon('other')).toBe('Shield');
    });
  });

  describe('extractFilePath', () => {
    it('extracts path from input', () => {
      const req: PermissionRequestInfo = {
        request_id: 'r1', tool_name: 'T', tool_use_id: 't1',
        title: '', description: '', input: { path: '/src/app.ts' },
        blocked_path: null, permission_suggestions: [],
      };
      expect(extractFilePath(req)).toBe('/src/app.ts');
    });

    it('returns null when no path', () => {
      const req: PermissionRequestInfo = {
        request_id: 'r1', tool_name: 'T', tool_use_id: 't1',
        title: '', description: '', input: {},
        blocked_path: null, permission_suggestions: [],
      };
      expect(extractFilePath(req)).toBeNull();
    });
  });

  describe('formatPermissionRule', () => {
    it('normalizes whitespace', () => {
      expect(formatPermissionRule('  allow   Read(*)  ')).toBe('allow Read(*)');
    });
  });

  describe('isDangerousPermission', () => {
    const base: PermissionRequestInfo = {
      request_id: 'r1', tool_name: '', tool_use_id: 't1',
      title: '', description: '', input: {},
      blocked_path: null, permission_suggestions: [],
    };

    it('detects dangerous tools', () => {
      expect(isDangerousPermission({ ...base, tool_name: 'Bash' })).toBe(true);
    });

    it('returns false for safe tools', () => {
      expect(isDangerousPermission({ ...base, tool_name: 'Read' })).toBe(false);
    });
  });
});
