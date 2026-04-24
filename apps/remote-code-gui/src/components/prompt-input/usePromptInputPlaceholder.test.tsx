import { describe, expect, it } from 'vitest';
import { renderHook } from '@testing-library/react';
import { usePromptInputPlaceholder } from './usePromptInputPlaceholder';

describe('usePromptInputPlaceholder', () => {
  it('默认返回普通提示', () => {
    const { result } = renderHook(() =>
      usePromptInputPlaceholder({
        hasSelection: false,
        isSearching: false,
        isLoading: false,
      }),
    );
    expect(result.current).toBe('输入需求，Shift+Enter 换行...');
  });

  it('hasSelection 时返回选中提示', () => {
    const { result } = renderHook(() =>
      usePromptInputPlaceholder({
        hasSelection: true,
        isSearching: false,
        isLoading: false,
      }),
    );
    expect(result.current).toBe('已选中代码片段，输入需求...');
  });

  it('isSearching 时返回搜索提示', () => {
    const { result } = renderHook(() =>
      usePromptInputPlaceholder({
        hasSelection: false,
        isSearching: true,
        isLoading: false,
      }),
    );
    expect(result.current).toBe('搜索历史命令...');
  });

  it('isLoading 时返回等待提示', () => {
    const { result } = renderHook(() =>
      usePromptInputPlaceholder({
        hasSelection: false,
        isSearching: false,
        isLoading: true,
      }),
    );
    expect(result.current).toBe('等待回复中...');
  });

  it('isSearching 优先于 isLoading', () => {
    const { result } = renderHook(() =>
      usePromptInputPlaceholder({
        hasSelection: false,
        isSearching: true,
        isLoading: true,
      }),
    );
    expect(result.current).toBe('搜索历史命令...');
  });

  it('有 modelName 时显示模型名称', () => {
    const { result } = renderHook(() =>
      usePromptInputPlaceholder({
        hasSelection: false,
        isSearching: false,
        isLoading: false,
        modelName: 'GPT-4',
      }),
    );
    expect(result.current).toBe('向 GPT-4 输入需求，Shift+Enter 换行...');
  });

  it('isLoading 优先于 hasSelection', () => {
    const { result } = renderHook(() =>
      usePromptInputPlaceholder({
        hasSelection: true,
        isSearching: false,
        isLoading: true,
      }),
    );
    expect(result.current).toBe('等待回复中...');
  });
});
