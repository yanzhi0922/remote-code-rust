import { describe, expect, it } from 'vitest';
import { formatTimestamp, formatDuration, truncateText, getRoleLabel, isErrorMessage, extractToolNames, formatTokenCount } from './messageUtils';
import type { ConversationEntry } from '../../lib/types';

describe('messageUtils', () => {
  describe('formatTimestamp', () => {
    it('formats Date object', () => {
      const date = new Date('2024-01-15T10:30:00Z');
      const result = formatTimestamp(date);
      expect(result).toBeTruthy();
      expect(typeof result).toBe('string');
    });

    it('formats ISO string', () => {
      const result = formatTimestamp('2024-01-15T10:30:00Z');
      expect(result).toBeTruthy();
    });
  });

  describe('formatDuration', () => {
    it('formats milliseconds', () => {
      expect(formatDuration(500)).toBe('500ms');
    });

    it('formats seconds', () => {
      expect(formatDuration(2500)).toBe('2.5s');
    });

    it('formats minutes and seconds', () => {
      expect(formatDuration(125_000)).toBe('2m 5s');
    });
  });

  describe('truncateText', () => {
    it('returns short text as-is', () => {
      expect(truncateText('hello')).toBe('hello');
    });

    it('truncates long text', () => {
      const long = 'a'.repeat(300);
      expect(truncateText(long)).toBe('a'.repeat(200) + '…');
    });

    it('respects custom maxLength', () => {
      expect(truncateText('hello world', 5)).toBe('hello…');
    });
  });

  describe('getRoleLabel', () => {
    it('returns correct labels', () => {
      expect(getRoleLabel('system')).toBe('系统');
      expect(getRoleLabel('user')).toBe('用户');
      expect(getRoleLabel('assistant')).toBe('助手');
      expect(getRoleLabel('tool')).toBe('工具');
    });
  });

  describe('isErrorMessage', () => {
    const base: ConversationEntry = {
      role: 'system',
      text: 'ok',
      content_blocks: [],
      tool_calls: [],
      tool_call_id: null,
      name: null,
      is_error: false,
    };

    it('detects error flag', () => {
      expect(isErrorMessage({ ...base, is_error: true })).toBe(true);
    });

    it('detects error text', () => {
      expect(isErrorMessage({ ...base, text: 'An error occurred' })).toBe(true);
    });

    it('returns false for normal messages', () => {
      expect(isErrorMessage(base)).toBe(false);
    });
  });

  describe('extractToolNames', () => {
    it('extracts tool names', () => {
      const entry: ConversationEntry = {
        role: 'assistant',
        text: '',
        content_blocks: [],
        tool_calls: [
          { id: '1', name: 'Read', input: {} },
          { id: '2', name: 'Write', input: {} },
        ],
        tool_call_id: null,
        name: null,
        is_error: false,
      };
      expect(extractToolNames(entry)).toEqual(['Read', 'Write']);
    });
  });

  describe('formatTokenCount', () => {
    it('formats small counts', () => {
      expect(formatTokenCount(500)).toBe('500');
    });

    it('formats thousands', () => {
      expect(formatTokenCount(2500)).toBe('2.5K');
    });

    it('formats millions', () => {
      expect(formatTokenCount(1_500_000)).toBe('1.50M');
    });
  });
});
