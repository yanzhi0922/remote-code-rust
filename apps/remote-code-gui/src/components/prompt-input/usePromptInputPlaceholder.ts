import { useMemo } from 'react';

/**
 * 动态 placeholder Hook。
 * 根据当前状态返回最合适的输入提示文本。
 */
export function usePromptInputPlaceholder(options: {
  hasSelection: boolean;
  isSearching: boolean;
  isLoading: boolean;
  modelName?: string;
}): string {
  const { hasSelection, isSearching, isLoading, modelName } = options;

  return useMemo(() => {
    if (isSearching) {
      return '搜索历史命令...';
    }
    if (isLoading) {
      return '等待回复中...';
    }
    if (hasSelection) {
      return '已选中代码片段，输入需求...';
    }
    if (modelName) {
      return `向 ${modelName} 输入需求，Shift+Enter 换行...`;
    }
    return '输入需求，Shift+Enter 换行...';
  }, [hasSelection, isSearching, isLoading, modelName]);
}
