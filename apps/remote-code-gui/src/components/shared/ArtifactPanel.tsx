/**
 * ArtifactPanel — 共享的产物库面板组件。
 *
 * 渲染产物列表，每项显示名称、文件名、大小，并提供下载动作。
 * 本地桌面端和远程 Web/PWA 端共用同一视觉语言。
 *
 * Props:
 * - title: 面板标题
 * - icon: 标题图标
 * - emptyText: 无产物时的提示文本
 * - items: 产物项列表
 * - buildDownloadUrl: 构建下载 URL 的回调
 */

import { Download, LoaderCircle, Share2 } from 'lucide-react';
import type { ReactNode } from 'react';
import { formatBytes } from './formatBytes';
import { PanelHint } from './ApprovalPanel';

export interface ArtifactItem {
  artifact_id: string;
  name: string;
  file_name: string;
  size_bytes: number;
}

export interface ArtifactPanelProps {
  title: string;
  icon: ReactNode;
  emptyText: string;
  items: ArtifactItem[];
  onDownload?: (artifact: ArtifactItem) => void | Promise<void>;
  onShare?: (artifact: ArtifactItem) => void | Promise<void>;
  buildDownloadUrl?: (artifactId: string) => string;
  downloadingId?: string | null;
  /** 为 true 时隐藏面板自带标题（用于外部已有标题的场景，如移动端 bottom sheet） */
  hideTitle?: boolean;
}

export function ArtifactPanel({
  title,
  icon,
  emptyText,
  items,
  onDownload,
  onShare,
  buildDownloadUrl,
  downloadingId,
  hideTitle,
}: ArtifactPanelProps) {
  return (
    <section className="rounded-[24px] border border-[#e0d6c6] bg-white px-4 py-4 shadow-[0_12px_30px_rgba(34,32,28,0.06)]">
      {!hideTitle && (
        <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
          {icon}
          {title}
        </div>
      )}
      <div className="mt-4 space-y-3">
        {items.length === 0 ? (
          <PanelHint>{emptyText}</PanelHint>
        ) : (
          items.map((artifact) => {
            const content = (
              <>
                <div className="min-w-0 text-left">
                  <div className="truncate text-sm font-medium text-slate-900">
                    {artifact.name}
                  </div>
                  <div className="mt-1 text-xs text-slate-500">
                    {artifact.file_name} • {formatBytes(artifact.size_bytes)}
                  </div>
                </div>
                {downloadingId === artifact.artifact_id ? (
                  <LoaderCircle size={16} className="mt-0.5 shrink-0 animate-spin text-slate-500" />
                ) : (
                  <Download size={16} className="mt-0.5 shrink-0 text-slate-500" />
                )}
              </>
            );

            if (onDownload) {
              return (
                <div
                  key={artifact.artifact_id}
                  className="flex items-start gap-2"
                >
                  <button
                    type="button"
                    onClick={() => {
                      void onDownload(artifact);
                    }}
                    disabled={downloadingId === artifact.artifact_id}
                    className="flex flex-1 items-start justify-between gap-3 rounded-2xl border border-[#ebe2d5] bg-[#faf7f1] px-3 py-3 transition-colors hover:bg-white disabled:cursor-not-allowed disabled:opacity-70"
                  >
                    {content}
                  </button>
                  {onShare && (
                    <button
                      type="button"
                      onClick={() => {
                        void onShare(artifact);
                      }}
                      disabled={downloadingId === artifact.artifact_id}
                      className="mt-1 inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-[#ebe2d5] bg-[#faf7f1] text-slate-500 transition-colors hover:bg-white hover:text-slate-700 disabled:cursor-not-allowed disabled:opacity-70"
                      aria-label="Share"
                    >
                      <Share2 size={14} />
                    </button>
                  )}
                </div>
              );
            }

            return (
              <a
                key={artifact.artifact_id}
                href={buildDownloadUrl?.(artifact.artifact_id) ?? '#'}
                className="flex items-start justify-between gap-3 rounded-2xl border border-[#ebe2d5] bg-[#faf7f1] px-3 py-3 transition-colors hover:bg-white"
              >
                {content}
              </a>
            );
          })
        )}
      </div>
    </section>
  );
}
