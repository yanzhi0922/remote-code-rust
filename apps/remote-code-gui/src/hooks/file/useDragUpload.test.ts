import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, act, cleanup } from '@testing-library/react';
import { useDragUpload } from './useDragUpload';

describe('useDragUpload', () => {
  afterEach(() => {
    cleanup();
  });

  it('returns initial non-dragging state', () => {
    const { result } = renderHook(() => useDragUpload());

    expect(result.current.isFileDragging).toBe(false);
    expect(result.current.handlers).toBeDefined();
  });

  it('sets dragging on drag enter', () => {
    const { result } = renderHook(() => useDragUpload());

    act(() => {
      result.current.handlers.onDragEnter({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as React.DragEvent);
    });

    expect(result.current.isFileDragging).toBe(true);
  });

  it('clears dragging on drag leave', () => {
    const { result } = renderHook(() => useDragUpload());

    act(() => {
      result.current.handlers.onDragEnter({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as React.DragEvent);
    });

    expect(result.current.isFileDragging).toBe(true);

    act(() => {
      result.current.handlers.onDragLeave({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as React.DragEvent);
    });

    expect(result.current.isFileDragging).toBe(false);
  });

  it('handles nested drag enter/leave with counter', () => {
    const { result } = renderHook(() => useDragUpload());

    // Two enters
    act(() => {
      result.current.handlers.onDragEnter({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as React.DragEvent);
    });
    act(() => {
      result.current.handlers.onDragEnter({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as React.DragEvent);
    });

    // One leave should not clear
    act(() => {
      result.current.handlers.onDragLeave({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as React.DragEvent);
    });

    expect(result.current.isFileDragging).toBe(true);

    // Second leave clears
    act(() => {
      result.current.handlers.onDragLeave({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as React.DragEvent);
    });

    expect(result.current.isFileDragging).toBe(false);
  });

  it('calls onFilesAdded on drop with valid files', async () => {
    const onFilesAdded = vi.fn();
    const { result } = renderHook(() =>
      useDragUpload({ onFilesAdded }),
    );

    const mockFile = new File(['content'], 'test.ts', { type: 'text/typescript' });
    const mockDataTransfer = {
      files: [mockFile] as unknown as FileList,
    };

    await act(async () => {
      result.current.handlers.onDrop({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
        nativeEvent: { dataTransfer: mockDataTransfer },
      } as unknown as React.DragEvent);
    });

    expect(onFilesAdded).toHaveBeenCalled();
    expect(onFilesAdded.mock.calls[0][0][0].name).toBe('test.ts');
  });

  it('does not call onFilesAdded when no callback', async () => {
    const { result } = renderHook(() => useDragUpload());

    const mockFile = new File(['content'], 'test.ts', { type: 'text/typescript' });
    const mockDataTransfer = {
      files: [mockFile] as unknown as FileList,
    };

    await act(async () => {
      result.current.handlers.onDrop({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
        nativeEvent: { dataTransfer: mockDataTransfer },
      } as unknown as React.DragEvent);
    });

    // Should not throw
  });

  it('clears dragging state on drop', async () => {
    const { result } = renderHook(() => useDragUpload());

    act(() => {
      result.current.handlers.onDragEnter({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as React.DragEvent);
    });

    expect(result.current.isFileDragging).toBe(true);

    await act(async () => {
      result.current.handlers.onDrop({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
        nativeEvent: { dataTransfer: { files: [] } },
      } as unknown as React.DragEvent);
    });

    expect(result.current.isFileDragging).toBe(false);
  });

  it('filters files by extension', async () => {
    const onFilesAdded = vi.fn();
    const { result } = renderHook(() =>
      useDragUpload({ supportedExts: ['.ts', '.tsx'], onFilesAdded }),
    );

    const validFile = new File(['code'], 'app.ts', { type: 'text/typescript' });
    const invalidFile = new File(['data'], 'data.csv', { type: 'text/csv' });
    const mockDataTransfer = {
      files: [validFile, invalidFile] as unknown as FileList,
    };

    await act(async () => {
      result.current.handlers.onDrop({
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
        nativeEvent: { dataTransfer: mockDataTransfer },
      } as unknown as React.DragEvent);
    });

    expect(onFilesAdded).toHaveBeenCalled();
    expect(onFilesAdded.mock.calls[0][0]).toHaveLength(1);
    expect(onFilesAdded.mock.calls[0][0][0].name).toBe('app.ts');
  });
});
