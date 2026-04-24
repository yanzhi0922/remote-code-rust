import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DiagnosticsPanel } from './DiagnosticsPanel';

afterEach(() => {
  cleanup();
});

describe('DiagnosticsPanel', () => {
  it('renders panel with header and run button', () => {
    render(<DiagnosticsPanel report={{ ok: true, issues: [], warnings: [] }} onRunDiagnostics={vi.fn()} />);
    expect(screen.getByTestId('diagnostics-panel')).toBeInTheDocument();
    expect(screen.getByText('诊断')).toBeInTheDocument();
    expect(screen.getByTestId('run-diagnostics')).toBeInTheDocument();
  });

  it('shows all-pass message when ok and no issues/warnings', () => {
    render(<DiagnosticsPanel report={{ ok: true, issues: [], warnings: [] }} onRunDiagnostics={vi.fn()} />);
    expect(screen.getByTestId('diagnostic-ok')).toBeInTheDocument();
    expect(screen.getByText('所有检查通过')).toBeInTheDocument();
  });

  it('renders issues in red', () => {
    render(
      <DiagnosticsPanel
        report={{ ok: false, issues: ['API key missing', 'Network error'], warnings: [] }}
        onRunDiagnostics={vi.fn()}
      />,
    );
    const issues = screen.getAllByTestId('diagnostic-issue');
    expect(issues).toHaveLength(2);
    expect(issues[0]).toHaveTextContent('API key missing');
    expect(issues[0].className).toContain('bg-red-50');
  });

  it('renders warnings in yellow', () => {
    render(
      <DiagnosticsPanel
        report={{ ok: false, issues: [], warnings: ['Deprecated API version'] }}
        onRunDiagnostics={vi.fn()}
      />,
    );
    const warnings = screen.getAllByTestId('diagnostic-warning');
    expect(warnings).toHaveLength(1);
    expect(warnings[0]).toHaveTextContent('Deprecated API version');
    expect(warnings[0].className).toContain('bg-yellow-50');
  });

  it('calls onRunDiagnostics when button clicked', () => {
    const onRun = vi.fn();
    render(<DiagnosticsPanel report={{ ok: true, issues: [], warnings: [] }} onRunDiagnostics={onRun} />);
    fireEvent.click(screen.getByTestId('run-diagnostics'));
    expect(onRun).toHaveBeenCalled();
  });

  it('shows spinner when loading', () => {
    render(
      <DiagnosticsPanel
        report={{ ok: true, issues: [], warnings: [] }}
        onRunDiagnostics={vi.fn()}
        loading={true}
      />,
    );
    expect(screen.getByTestId('diagnostics-spinner')).toBeInTheDocument();
    expect(screen.getByTestId('run-diagnostics')).toBeDisabled();
  });

  it('does not show all-pass when issues exist', () => {
    render(
      <DiagnosticsPanel
        report={{ ok: false, issues: ['Error'], warnings: [] }}
        onRunDiagnostics={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('diagnostic-ok')).not.toBeInTheDocument();
  });
});
