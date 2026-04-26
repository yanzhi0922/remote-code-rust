import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/react';
import { DiagnosticsPanel, type DiagnosticsReport } from './DiagnosticsPanel';

describe('DiagnosticsPanel', () => {
  afterEach(() => { cleanup(); });

  it('renders diagnostics panel with run button', () => {
    const report: DiagnosticsReport = { ok: true, issues: [], warnings: [] };
    const { getByTestId, getByText } = render(
      <DiagnosticsPanel report={report} onRunDiagnostics={() => {}} />,
    );
    expect(getByTestId('diagnostics-panel')).toBeInTheDocument();
    expect(getByTestId('run-diagnostics')).toBeInTheDocument();
    expect(getByText('运行诊断')).toBeInTheDocument();
  });

  it('shows spinner when loading', () => {
    const report: DiagnosticsReport = { ok: true, issues: [], warnings: [] };
    const { getByTestId, getByText } = render(
      <DiagnosticsPanel report={report} onRunDiagnostics={() => {}} loading />,
    );
    expect(getByTestId('diagnostics-spinner')).toBeInTheDocument();
    expect(getByText('运行中...')).toBeInTheDocument();
  });

  it('renders issues', () => {
    const report: DiagnosticsReport = { ok: false, issues: ['API key missing'], warnings: [] };
    const { getByText } = render(
      <DiagnosticsPanel report={report} onRunDiagnostics={() => {}} />,
    );
    expect(getByText('API key missing')).toBeInTheDocument();
  });

  it('renders warnings', () => {
    const report: DiagnosticsReport = { ok: true, issues: [], warnings: ['Slow network'] };
    const { getByText } = render(
      <DiagnosticsPanel report={report} onRunDiagnostics={() => {}} />,
    );
    expect(getByText('Slow network')).toBeInTheDocument();
  });

  it('calls onRunDiagnostics when button clicked', () => {
    const fn = vi.fn();
    const report: DiagnosticsReport = { ok: true, issues: [], warnings: [] };
    const { getByTestId } = render(
      <DiagnosticsPanel report={report} onRunDiagnostics={fn} />,
    );
    fireEvent.click(getByTestId('run-diagnostics'));
    expect(fn).toHaveBeenCalled();
  });
});
