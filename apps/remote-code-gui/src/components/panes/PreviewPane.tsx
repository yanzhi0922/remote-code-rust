import { useState } from 'react';
import { Eye, FileCode, FileText, Image } from 'lucide-react';
import MarkdownRenderer from '../chat/MarkdownRenderer';

interface PreviewPaneProps {
  content?: string;
  url?: string;
  language?: string;
  className?: string;
}

function detectFileType(filename?: string, language?: string): 'markdown' | 'code' | 'image' | 'html' | 'text' {
  if (language === 'markdown' || language === 'md') return 'markdown';
  if (language === 'html') return 'html';
  if (!filename) return 'text';
  const ext = filename.split('.').pop()?.toLowerCase() ?? '';
  if (['md', 'mdx', 'markdown'].includes(ext)) return 'markdown';
  if (['html', 'htm'].includes(ext)) return 'html';
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'ico'].includes(ext)) return 'image';
  if (['ts', 'tsx', 'js', 'jsx', 'rs', 'py', 'go', 'java', 'c', 'cpp', 'h', 'toml', 'yaml', 'yml', 'json'].includes(ext))
    return 'code';
  return 'text';
}

function FileIcon({ type }: { type: string }) {
  switch (type) {
    case 'code':
      return <FileCode size={14} className="text-rc-accent-primary" />;
    case 'image':
      return <Image size={14} className="text-rc-accent-success" />;
    default:
      return <FileText size={14} className="text-rc-text-secondary" />;
  }
}

export function PreviewPane({ content, url, language, className = '' }: PreviewPaneProps) {
  const [tab, setTab] = useState<'preview' | 'source'>('preview');
  const fileType = detectFileType(url, language);

  const showContent = content ?? '';
  const hasContent = showContent.length > 0;

  return (
    <div className={`flex h-full flex-col bg-rc-bg-primary ${className}`}>
      <div className="flex items-center justify-between border-b border-rc-border-primary px-3 py-1.5">
        <div className="flex items-center gap-2">
          <Eye size={14} className="text-rc-text-secondary" />
          <span className="text-xs font-medium text-rc-text-primary">Preview</span>
          {url && (
            <span className="max-w-[200px] truncate text-2xs text-rc-text-tertiary" title={url}>
              {url.split('/').pop()}
            </span>
          )}
        </div>
        {hasContent && fileType !== 'image' && (
          <div className="flex rounded-md border border-rc-border-primary text-2xs">
            <button
              type="button"
              onClick={() => setTab('preview')}
              className={`px-2 py-0.5 ${tab === 'preview' ? 'bg-rc-bg-active text-rc-text-primary' : 'text-rc-text-tertiary hover:bg-rc-bg-hover'}`}
            >
              预览
            </button>
            <button
              type="button"
              onClick={() => setTab('source')}
              className={`px-2 py-0.5 ${tab === 'source' ? 'bg-rc-bg-active text-rc-text-primary' : 'text-rc-text-tertiary hover:bg-rc-bg-hover'}`}
            >
              源码
            </button>
          </div>
        )}
      </div>
      <div className="flex-1 overflow-auto">
        {!hasContent ? (
          <div className="flex h-full items-center justify-center p-3 text-rc-text-tertiary text-xs">
            <div className="text-center">
              <Eye size={24} className="mx-auto mb-2 opacity-40" />
              <p>支持 Markdown、代码、图片等内容预览</p>
              <p className="mt-1 text-2xs">Agent 输出文件时可自动展示预览</p>
            </div>
          </div>
        ) : tab === 'source' || fileType === 'code' || fileType === 'text' ? (
          <pre className="overflow-auto p-3 font-mono text-xs leading-5 text-rc-text-inverse">
            <code>{showContent}</code>
          </pre>
        ) : fileType === 'markdown' ? (
          <div className="p-3">
            <MarkdownRenderer content={showContent} />
          </div>
        ) : fileType === 'html' ? (
          <iframe
            srcDoc={showContent}
            className="h-full w-full border-0"
            sandbox="allow-same-origin"
            title="HTML Preview"
          />
        ) : fileType === 'image' ? (
          <div className="flex h-full items-center justify-center p-3">
            {url ? (
              <img src={url} alt="Preview" className="max-h-full max-w-full object-contain" />
            ) : (
              <div className="text-center text-rc-text-tertiary text-xs">
                <Image size={24} className="mx-auto mb-2 opacity-40" />
                <p>图片预览需要 URL 路径</p>
              </div>
            )}
          </div>
        ) : null}
      </div>
    </div>
  );
}