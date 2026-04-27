/**
 * 拖拽上传 Hook — 管理文件拖拽状态和文件类型过滤
 * Drag upload hook — manages file drag state and file type filtering
 *
 * Adapted from AionUi useDragUpload pattern, simplified for Tauri.
 */

import { useCallback, useRef, useState } from 'react';

export interface FileMetadata {
  name: string;
  path: string;
  size: number;
  type: string;
}

export interface UseDragUploadOptions {
  /** 支持的文件扩展名列表，空数组表示支持所有文件 */
  supportedExts?: string[];
  /** 文件添加回调 */
  onFilesAdded?: (files: FileMetadata[]) => void;
}

const SUPPORTED_EXTS_DEFAULT: string[] = [];

function isSupportedFile(fileName: string, supportedExts: string[]): boolean {
  if (supportedExts.length === 0) return true;
  const ext = fileName.lastIndexOf('.') >= 0 ? fileName.slice(fileName.lastIndexOf('.')).toLowerCase() : '';
  return supportedExts.includes(ext);
}

/**
 * @example
 * ```tsx
 * const { isFileDragging, handlers } = useDragUpload({
 *   supportedExts: ['.ts', '.tsx', '.js'],
 *   onFilesAdded: (files) => console.log('Dropped:', files),
 * });
 * // <div {...handlers}>Drop files here</div>
 * ```
 */
export function useDragUpload({
  supportedExts = SUPPORTED_EXTS_DEFAULT,
  onFilesAdded,
}: UseDragUploadOptions = {}) {
  const [isFileDragging, setIsFileDragging] = useState(false);
  const dragCounter = useRef(0);

  const handleDragOver = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      // Drag-over fires continuously; only set state once on transition.
      if (dragCounter.current === 0) {
        dragCounter.current = 1;
        setIsFileDragging(true);
      }
    },
    [],
  );

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounter.current += 1;
    setIsFileDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounter.current -= 1;
    if (dragCounter.current <= 0) {
      dragCounter.current = 0;
      setIsFileDragging(false);
    }
  }, []);

  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();

      dragCounter.current = 0;
      setIsFileDragging(false);

      if (!onFilesAdded) return;

      try {
        const droppedFiles = e.nativeEvent.dataTransfer?.files;
        if (!droppedFiles || droppedFiles.length === 0) return;

        const validFiles: FileMetadata[] = [];

        for (let i = 0; i < droppedFiles.length; i++) {
          const file = droppedFiles[i];
          if (isSupportedFile(file.name, supportedExts)) {
            validFiles.push({
              name: file.name,
              path: (file as File & { path?: string }).path || file.name,
              size: file.size,
              type: file.type,
            });
          }
        }

        if (validFiles.length > 0) {
          onFilesAdded(validFiles);
        }
      } catch (err) {
        console.error('Failed to process dropped files:', err);
      }
    },
    [onFilesAdded, supportedExts],
  );

  return {
    isFileDragging,
    handlers: {
      onDragOver: handleDragOver,
      onDragEnter: handleDragEnter,
      onDragLeave: handleDragLeave,
      onDrop: handleDrop,
    },
  };
}
