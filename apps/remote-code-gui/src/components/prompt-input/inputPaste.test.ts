import { describe, expect, it } from 'vitest';
import {
  hasImageInClipboard,
  extractImageFiles,
  formatPastedText,
} from './inputPaste';

describe('inputPaste', () => {
  describe('hasImageInClipboard', () => {
    it('包含图片类型时返回 true', () => {
      const items = [
        { type: 'text/plain' },
        { type: 'image/png' },
      ] as unknown as DataTransferItemList;
      const event = {
        clipboardData: { items },
      } as ClipboardEvent;
      expect(hasImageInClipboard(event)).toBe(true);
    });

    it('不包含图片类型时返回 false', () => {
      const items = [
        { type: 'text/plain' },
        { type: 'text/html' },
      ] as unknown as DataTransferItemList;
      const event = {
        clipboardData: { items },
      } as ClipboardEvent;
      expect(hasImageInClipboard(event)).toBe(false);
    });

    it('clipboardData 为 null 时返回 false', () => {
      const event = { clipboardData: null } as unknown as ClipboardEvent;
      expect(hasImageInClipboard(event)).toBe(false);
    });
  });

  describe('extractImageFiles', () => {
    it('提取图片文件', () => {
      const mockFile = new File([''], 'test.png', { type: 'image/png' });
      const items = [
        { type: 'text/plain', getAsFile: () => null },
        { type: 'image/png', getAsFile: () => mockFile },
      ] as unknown as DataTransferItemList;
      const files = extractImageFiles(items);
      expect(files).toHaveLength(1);
      expect(files[0].name).toBe('test.png');
    });

    it('无图片时返回空数组', () => {
      const items = [
        { type: 'text/plain', getAsFile: () => null },
      ] as unknown as DataTransferItemList;
      const files = extractImageFiles(items);
      expect(files).toHaveLength(0);
    });
  });

  describe('formatPastedText', () => {
    it('去除 ANSI 转义序列', () => {
      const text = '\x1b[31mHello\x1b[0m World';
      expect(formatPastedText(text)).toBe('Hello World');
    });

    it('短文本原样返回', () => {
      expect(formatPastedText('hello')).toBe('hello');
    });

    it('超长文本截断并添加省略号', () => {
      const longText = 'a'.repeat(15000);
      const result = formatPastedText(longText);
      expect(result.length).toBe(10001);
      expect(result.endsWith('…')).toBe(true);
    });

    it('正常长度文本不截断', () => {
      const text = 'a'.repeat(5000);
      expect(formatPastedText(text)).toBe(text);
    });
  });
});
