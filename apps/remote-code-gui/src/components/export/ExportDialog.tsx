/**
 * ExportDialog — 导出对话框组件。
 *
 * 格式选择（JSON / NDJSON），每种格式附带说明，导出成功提示。
 */

import { useState } from 'react';
import { Download, X } from 'lucide-react';

type ExportFormat = 'json' | 'ndjson';

export interface ExportDialogProps {
  visible: boolean;
  sessionId: string;
  onExport: (format: ExportFormat) => void;
  onClose: () => void;
}

const FORMAT_INFO: Record<ExportFormat, { label: string; desc: string }> = {
  json: {
    label: 'JSON',
    desc: '标准 JSON 格式，包含完整的会话数据和元信息。适合导入和数据分析。',
  },
  ndjson: {
    label: 'NDJSON',
    desc: '每行一条 JSON 记录的流式格式。适合大数据量处理和流式读取。',
  },
};

export function ExportDialog({ visible, sessionId, onExport, onClose }: ExportDialogProps) {
  const [format, setFormat] = useState<ExportFormat>('json');
  const [exported, setExported] = useState(false);

  if (!visible) return null;

  const handleExport = () => {
    onExport(format);
    setExported(true);
  };

  const handleClose = () => {
    setExported(false);
    setFormat('json');
    onClose();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      data-testid="export-overlay"
    >
      <div className="w-full max-w-md rounded-2xl bg-white p-6 shadow-xl" data-testid="export-dialog">
        {/* Header */}
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold text-slate-900">导出会话</h2>
          <button
            onClick={handleClose}
            className="rounded-lg p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
            data-testid="export-close"
            aria-label="关闭"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Session ID */}
        <p className="mt-2 text-xs text-slate-400" data-testid="export-session-id">
          会话: {sessionId.slice(0, 12)}...
        </p>

        {exported ? (
          /* Success message */
          <div className="mt-6 text-center" data-testid="export-success">
            <Download className="mx-auto h-8 w-8 text-green-500" />
            <p className="mt-3 text-sm font-medium text-slate-900">导出成功！</p>
            <p className="mt-1 text-xs text-slate-500">文件已开始下载。</p>
            <button
              onClick={handleClose}
              className="mt-4 rounded-xl bg-slate-900 px-4 py-2 text-sm text-white hover:bg-slate-800"
            >
              关闭
            </button>
          </div>
        ) : (
          <>
            {/* Format selection */}
            <div className="mt-4 space-y-3" data-testid="format-options">
              {(Object.keys(FORMAT_INFO) as ExportFormat[]).map((fmt) => (
                <button
                  key={fmt}
                  onClick={() => setFormat(fmt)}
                  className={`w-full rounded-xl border-2 px-4 py-3 text-left transition-colors ${
                    format === fmt
                      ? 'border-blue-500 bg-blue-50'
                      : 'border-slate-200 bg-slate-50 hover:border-slate-300'
                  }`}
                  data-testid={`format-${fmt}`}
                >
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-medium text-slate-900">
                      {FORMAT_INFO[fmt].label}
                    </span>
                    {format === fmt && (
                      <span className="h-3 w-3 rounded-full bg-blue-500" data-testid={`format-check-${fmt}`} />
                    )}
                  </div>
                  <p className="mt-1 text-xs text-slate-500">{FORMAT_INFO[fmt].desc}</p>
                </button>
              ))}
            </div>

            {/* Export button */}
            <button
              onClick={handleExport}
              className="mt-4 flex w-full items-center justify-center gap-2 rounded-xl bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
              data-testid="export-button"
            >
              <Download className="h-4 w-4" />
              导出 {FORMAT_INFO[format].label}
            </button>
          </>
        )}
      </div>
    </div>
  );
}
