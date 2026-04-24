/**
 * DiagnosticsPanel — 诊断面板组件。
 *
 * 显示运行诊断按钮、问题列表（红色）、警告列表（黄色）、通过项（绿色勾）。
 * 支持加载中 spinner。
 */

import { CheckCircle, AlertTriangle, AlertCircle, Loader2 } from 'lucide-react';

export interface DiagnosticsReport {
  ok: boolean;
  issues: string[];
  warnings: string[];
}

export interface DiagnosticsPanelProps {
  report: DiagnosticsReport;
  onRunDiagnostics: () => void;
  loading?: boolean;
}

export function DiagnosticsPanel({ report, onRunDiagnostics, loading = false }: DiagnosticsPanelProps) {
  return (
    <div className="rounded-2xl border border-slate-200 bg-white p-4" data-testid="diagnostics-panel">
      {/* Header with run button */}
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-slate-900">诊断</h3>
        <button
          onClick={onRunDiagnostics}
          disabled={loading}
          className="flex items-center gap-2 rounded-xl bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-700 disabled:opacity-50"
          data-testid="run-diagnostics"
        >
          {loading ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" data-testid="diagnostics-spinner" />
          ) : null}
          {loading ? '运行中...' : '运行诊断'}
        </button>
      </div>

      {/* Results */}
      <div className="mt-4 space-y-2" data-testid="diagnostics-results">
        {/* Issues */}
        {report.issues.map((issue, i) => (
          <div
            key={`issue-${i}`}
            className="flex items-start gap-2 rounded-xl bg-red-50 px-3 py-2 text-sm text-red-700"
            data-testid="diagnostic-issue"
          >
            <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-red-500" />
            <span>{issue}</span>
          </div>
        ))}

        {/* Warnings */}
        {report.warnings.map((warning, i) => (
          <div
            key={`warning-${i}`}
            className="flex items-start gap-2 rounded-xl bg-yellow-50 px-3 py-2 text-sm text-yellow-700"
            data-testid="diagnostic-warning"
          >
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-yellow-500" />
            <span>{warning}</span>
          </div>
        ))}

        {/* All OK */}
        {report.ok && report.issues.length === 0 && report.warnings.length === 0 && (
          <div
            className="flex items-center gap-2 rounded-xl bg-green-50 px-3 py-2 text-sm text-green-700"
            data-testid="diagnostic-ok"
          >
            <CheckCircle className="h-4 w-4 text-green-500" />
            <span>所有检查通过</span>
          </div>
        )}
      </div>
    </div>
  );
}
