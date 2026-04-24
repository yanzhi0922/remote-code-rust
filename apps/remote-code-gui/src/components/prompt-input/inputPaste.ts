/**
 * 剪贴板粘贴工具函数。
 * 处理图片检测、文件提取和文本格式化。
 */

/** 检测剪贴板事件中是否包含图片数据 */
export function hasImageInClipboard(e: ClipboardEvent): boolean {
  const items = e.clipboardData?.items;
  if (!items) return false;
  for (let i = 0; i < items.length; i++) {
    if (items[i].type.startsWith('image/')) {
      return true;
    }
  }
  return false;
}

/** 从 DataTransferItemList 中提取所有图片文件 */
export function extractImageFiles(items: DataTransferItemList): File[] {
  const files: File[] = [];
  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (item.type.startsWith('image/')) {
      const file = item.getAsFile();
      if (file) {
        files.push(file);
      }
    }
  }
  return files;
}

/** ANSI 转义序列正则 */
const ANSI_REGEX = /\x1b\[[0-9;]*[a-zA-Z]/g;

/** 粘贴文本最大长度 */
const MAX_PASTE_LENGTH = 10000;

/** 格式化粘贴文本：去除 ANSI 转义序列并截断超长文本 */
export function formatPastedText(text: string): string {
  const cleaned = text.replace(ANSI_REGEX, '');
  if (cleaned.length > MAX_PASTE_LENGTH) {
    return cleaned.slice(0, MAX_PASTE_LENGTH) + '…';
  }
  return cleaned;
}
