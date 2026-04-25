import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { renderHook, act, cleanup } from '@testing-library/react';
import { useUploadState, trackUpload, resetUploadState } from './useUploadState';

describe('useUploadState', () => {
  beforeEach(() => {
    resetUploadState();
  });

  afterEach(() => {
    resetUploadState();
    cleanup();
  });

  it('returns initial idle state', () => {
    const { result } = renderHook(() => useUploadState());

    expect(result.current.activeCount).toBe(0);
    expect(result.current.isUploading).toBe(false);
    expect(result.current.overallPercent).toBe(0);
  });

  it('tracks an upload', () => {
    const { result } = renderHook(() => useUploadState());

    act(() => {
      trackUpload(1024);
    });

    expect(result.current.activeCount).toBe(1);
    expect(result.current.isUploading).toBe(true);
    expect(result.current.overallPercent).toBe(0);
  });

  it('updates progress', () => {
    const { result } = renderHook(() => useUploadState());

    let uploadHandle: ReturnType<typeof trackUpload>;
    act(() => {
      uploadHandle = trackUpload(1024);
    });

    act(() => {
      uploadHandle.onProgress(50);
    });

    expect(result.current.overallPercent).toBe(50);
  });

  it('finishes an upload', () => {
    const { result } = renderHook(() => useUploadState());

    let uploadHandle: ReturnType<typeof trackUpload>;
    act(() => {
      uploadHandle = trackUpload(1024);
    });

    act(() => {
      uploadHandle.finish();
    });

    expect(result.current.activeCount).toBe(0);
    expect(result.current.isUploading).toBe(false);
  });

  it('tracks multiple uploads', () => {
    const { result } = renderHook(() => useUploadState());

    act(() => {
      trackUpload(1024);
      trackUpload(2048);
    });

    expect(result.current.activeCount).toBe(2);
  });

  it('filters by source', () => {
    const { result } = renderHook(() => useUploadState('sendbox'));

    act(() => {
      trackUpload(1024, 'sendbox');
      trackUpload(2048, 'workspace');
    });

    expect(result.current.activeCount).toBe(1);
  });

  it('calculates weighted progress', () => {
    const { result } = renderHook(() => useUploadState());

    let handle1: ReturnType<typeof trackUpload>;

    act(() => {
      handle1 = trackUpload(1000);
      trackUpload(3000);
    });

    act(() => {
      handle1.onProgress(100); // 1000 bytes done
    });

    // Weighted: (1000 * 1.0 + 3000 * 0) / 4000 = 25%
    expect(result.current.overallPercent).toBe(25);
  });

  it('clamps progress to 0-100', () => {
    const { result } = renderHook(() => useUploadState());

    let handle: ReturnType<typeof trackUpload>;
    act(() => {
      handle = trackUpload(1024);
    });

    act(() => {
      handle.onProgress(150);
    });

    expect(result.current.overallPercent).toBe(100);
  });
});
