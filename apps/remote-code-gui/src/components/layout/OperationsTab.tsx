import { Archive, Download, RefreshCw, ShieldCheck, Stethoscope } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { formatSensitivePath } from '../../lib/utils';
import type { DoctorReportInfo, SessionExportFormat, SessionSummary } from '../../lib/types';
import * as tauri from '../../lib/tauri';
import { useAppStore } from '../../stores/useAppStore';
import { CodexOperationsPanel } from './CodexOperationsPanel';

const EXPORT_FORMATS: Array<{ value: SessionExportFormat; label: string }> = [
  { value: 'json', label: 'JSON bundle' },
  { value: 'ndjson', label: 'NDJSON transcript' },
];

function DoctorList({
  title,
  items,
  tone,
}: {
  title: string;
  items: string[];
  tone: 'issue' | 'warning';
}) {
  if (items.length === 0) return null;
  return (
    <div
      className={`rounded-lg border px-4 py-4 ${
        tone === 'issue'
          ? 'border-rc-accent-error-border bg-rc-accent-error-bg text-rc-accent-error'
          : 'border-rc-accent-warning-border bg-rc-accent-warning-bg text-rc-accent-warning'
      }`}
    >
      <div className="text-sm font-semibold">{title}</div>
      <div className="mt-2 space-y-1.5 text-sm">
        {items.map((item) => (
          <div key={item}>- {item}</div>
        ))}
      </div>
    </div>
  );
}

function describeSession(session: SessionSummary): string {
  return `${session.title} · ${session.provider_name}${session.model ? ` / ${session.model}` : ''}`;
}

export function OperationsTab() {
  const { t } = useTranslation();
  const privacyMode = useAppStore((state) => state.workspacePrivacyMode);
  const sessions = useAppStore((state) => state.sessions);
  const archivedSessions = useAppStore((state) => state.archivedSessions);
  const activeSessionId = useAppStore((state) => state.activeSessionId);
  const refreshSessions = useAppStore((state) => state.refreshSessions);
  const loadArchivedSessions = useAppStore((state) => state.loadArchivedSessions);

  const [doctor, setDoctor] = useState<DoctorReportInfo | null>(null);
  const [doctorLoading, setDoctorLoading] = useState(false);
  const [doctorError, setDoctorError] = useState<string | null>(null);
  const [probeProvider, setProbeProvider] = useState(false);
  const [probeNetwork, setProbeNetwork] = useState(false);
  const [includeEnvProviders, setIncludeEnvProviders] = useState(true);

  const [selectedSessionId, setSelectedSessionId] = useState<string>('');
  const [exportFormat, setExportFormat] = useState<SessionExportFormat>('json');
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [includeDiagnosticLogs, setIncludeDiagnosticLogs] = useState(true);
  const [includeDiagnosticSettings, setIncludeDiagnosticSettings] = useState(false);
  const [diagnosticExporting, setDiagnosticExporting] = useState(false);
  const [diagnosticError, setDiagnosticError] = useState<string | null>(null);
  const [diagnosticPath, setDiagnosticPath] = useState<string | null>(null);

  const allSessions = useMemo(() => {
    const deduped = new Map<string, SessionSummary>();
    [...sessions, ...archivedSessions].forEach((session) => {
      deduped.set(session.id, session);
    });
    return [...deduped.values()];
  }, [archivedSessions, sessions]);

  useEffect(() => {
    void Promise.all([refreshSessions(), loadArchivedSessions()]);
  }, [loadArchivedSessions, refreshSessions]);

  useEffect(() => {
    if (selectedSessionId) return;
    setSelectedSessionId(activeSessionId ?? allSessions[0]?.id ?? '');
  }, [activeSessionId, allSessions, selectedSessionId]);

  useEffect(() => {
    let cancelled = false;
    setDoctorLoading(true);
    setDoctorError(null);
    void tauri
      .runDoctorReport(false, false, false, true)
      .then((report) => {
        if (!cancelled) {
          setDoctor(report);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setDoctorError(typeof error === 'string' ? error : String(error));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setDoctorLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const runDoctor = async () => {
    setDoctorLoading(true);
    setDoctorError(null);
    try {
      const report = await tauri.runDoctorReport(probeNetwork, probeProvider, false, includeEnvProviders);
      setDoctor(report);
    } catch (error) {
      setDoctorError(typeof error === 'string' ? error : String(error));
    } finally {
      setDoctorLoading(false);
    }
  };

  const exportSession = async () => {
    if (!selectedSessionId) return;
    setExporting(true);
    setExportError(null);
    try {
      const result = await tauri.exportSessionBundle(selectedSessionId, exportFormat);
      setExportPath(result.path);
    } catch (error) {
      setExportError(typeof error === 'string' ? error : String(error));
    } finally {
      setExporting(false);
    }
  };

  const exportDiagnostics = async () => {
    setDiagnosticExporting(true);
    setDiagnosticError(null);
    try {
      const result = await tauri.exportDiagnosticBundle({
        includeLogs: includeDiagnosticLogs,
        includeSettings: includeDiagnosticSettings,
      });
      setDiagnosticPath(result.path);
    } catch (error) {
      setDiagnosticError(typeof error === 'string' ? error : String(error));
    } finally {
      setDiagnosticExporting(false);
    }
  };

  return (
    <div className="space-y-6">
      <CodexOperationsPanel />

      <section className="space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-base font-semibold text-rc-text-primary">Doctor</h3>
            <p className="mt-1 text-sm text-rc-text-tertiary">
              {t('operations.doctorDesc')}
            </p>
          </div>
          <button
            onClick={() => {
              void runDoctor();
            }}
            disabled={doctorLoading}
            className="inline-flex items-center gap-2 rounded-md border border-rc-border-primary bg-rc-bg-surface px-4 py-2 text-sm font-medium text-rc-text-primary transition-colors hover:bg-rc-bg-hover disabled:cursor-not-allowed disabled:opacity-60"
          >
            <RefreshCw size={14} className={doctorLoading ? 'animate-spin' : ''} />
            {doctorLoading ? t('operations.diagnosing') : t('operations.reDiagnose')}
          </button>
        </div>

        <div className="grid gap-3 md:grid-cols-3">
          <label className="flex items-center gap-3 rounded-md bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary">
            <input
              type="checkbox"
              checked={probeProvider}
              onChange={(event) => setProbeProvider(event.target.checked)}
            />
            <span>{t('operations.probeProvider')}</span>
          </label>
          <label className="flex items-center gap-3 rounded-md bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary">
            <input
              type="checkbox"
              checked={probeNetwork}
              onChange={(event) => setProbeNetwork(event.target.checked)}
            />
            <span>{t('operations.probeNetwork')}</span>
          </label>
          <label className="flex items-center gap-3 rounded-md bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary">
            <input
              type="checkbox"
              checked={includeEnvProviders}
              onChange={(event) => setIncludeEnvProviders(event.target.checked)}
            />
            <span>{t('operations.showEnvVarProvider')}</span>
          </label>
        </div>

        {doctorError && (
          <div className="rounded-lg border border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-3 text-sm text-rc-accent-error">
            {doctorError}
          </div>
        )}

        {doctor && (
          <div className="space-y-4">
            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
              <div className="rounded-lg border border-rc-border-primary bg-rc-bg-surface px-4 py-4">
                <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
                  <Stethoscope size={15} />
                  Readiness
                </div>
                <div
                  className={`mt-3 inline-flex rounded-full px-3 py-1 text-xs font-semibold ${
                    doctor.ok ? 'bg-rc-accent-success-bg text-rc-accent-success' : 'bg-rc-accent-error-bg text-rc-accent-error'
                  }`}
                >
                  {doctor.ok ? 'READY' : 'NOT READY'}
                </div>
                <div className="mt-3 text-xs text-rc-text-tertiary">
                  {doctor.provider.name} · {doctor.provider.protocol}
                </div>
              </div>

              <div className="rounded-lg border border-rc-border-primary bg-rc-bg-surface px-4 py-4">
                <div className="text-sm font-semibold text-rc-text-primary">Runtime</div>
                <div className="mt-3 text-sm text-rc-text-primary">{doctor.runtime.permission_mode}</div>
                <div className="mt-2 text-xs text-rc-text-tertiary">
                  session {doctor.runtime.session_name ?? '(auto)'}
                </div>
                <div className="mt-1 text-xs text-rc-text-tertiary">{doctor.runtime.version}</div>
              </div>

              <div className="rounded-lg border border-rc-border-primary bg-rc-bg-surface px-4 py-4">
                <div className="text-sm font-semibold text-rc-text-primary">Provider</div>
                <div className="mt-3 text-sm text-rc-text-primary">
                  {doctor.provider.model ?? '(missing model)'}
                </div>
                <div className="mt-2 text-xs text-rc-text-tertiary">
                  auth {doctor.provider.auth_source ?? '(missing)'}
                </div>
                <div className="mt-1 text-xs text-rc-text-tertiary">
                  ctx {doctor.provider.context_window_tokens} / out {doctor.provider.output_reserve_tokens}
                </div>
              </div>

              <div className="rounded-lg border border-rc-border-primary bg-rc-bg-surface px-4 py-4">
                <div className="flex items-center gap-2 text-sm font-semibold text-rc-text-primary">
                  <ShieldCheck size={15} />
                  Surfaces
                </div>
                <div className="mt-3 text-sm text-rc-text-primary">
                  tools {doctor.tools.builtin_tools} · rules {doctor.permissions.layered_rules}
                </div>
                <div className="mt-2 text-xs text-rc-text-tertiary">
                  skills {doctor.extensions.skills} · plugins {doctor.extensions.plugins} · disabled{' '}
                  {doctor.extensions.disabled_plugins}
                </div>
                <div className="mt-1 text-xs text-rc-text-tertiary">
                  mcp {doctor.extensions.managed_mcp_servers + doctor.extensions.plugin_mcp_servers}
                </div>
              </div>
            </div>

            <DoctorList title="Issues" items={doctor.issues} tone="issue" />
            <DoctorList title="Warnings" items={doctor.warnings} tone="warning" />

            <details className="rounded-lg border border-rc-border-primary bg-rc-bg-surface px-4 py-4">
              <summary className="cursor-pointer text-sm font-semibold text-rc-text-primary">
                详细信息
              </summary>
              <div className="mt-4 grid gap-4 lg:grid-cols-2">
                <div className="space-y-2 text-sm text-rc-text-secondary">
                  <div>
                    <span className="font-medium text-rc-text-primary">CWD:</span>{' '}
                    {formatSensitivePath(doctor.runtime.cwd, privacyMode)}
                  </div>
                  <div>
                    <span className="font-medium text-rc-text-primary">Profile:</span>{' '}
                    {formatSensitivePath(doctor.runtime.profile_dir, privacyMode)}
                  </div>
                  <div>
                    <span className="font-medium text-rc-text-primary">Setting sources:</span>{' '}
                    {doctor.runtime.setting_sources.length > 0
                      ? doctor.runtime.setting_sources.join(', ')
                      : '(defaults)'}
                  </div>
                  <div>
                    <span className="font-medium text-rc-text-primary">Allowed setting sources:</span>{' '}
                    {doctor.runtime.allowed_setting_sources.length > 0
                      ? doctor.runtime.allowed_setting_sources.join(', ')
                      : '(none)'}
                  </div>
                  <div>
                    <span className="font-medium text-rc-text-primary">Settings files:</span>{' '}
                    {doctor.runtime.settings_files.length > 0
                      ? doctor.runtime.settings_files
                          .map((path) => formatSensitivePath(path, privacyMode))
                          .join(', ')
                      : '(auto discovery only)'}
                  </div>
                  <div>
                    <span className="font-medium text-rc-text-primary">Allowed tools:</span>{' '}
                    {doctor.tools.allowed_tools.length > 0 ? doctor.tools.allowed_tools.join(', ') : '(all)'}
                  </div>
                  <div>
                    <span className="font-medium text-rc-text-primary">Denied tools:</span>{' '}
                    {doctor.tools.disallowed_tools.length > 0
                      ? doctor.tools.disallowed_tools.join(', ')
                      : '(none)'}
                  </div>
                </div>

                <div className="space-y-3 text-sm text-rc-text-secondary">
                  {doctor.provider.probe && (
                    <div className="rounded-md bg-rc-bg-tertiary px-3 py-3">
                      <div className="font-medium text-rc-text-primary">Provider probe</div>
                      <div className="mt-1 text-xs text-rc-text-tertiary">
                        {doctor.provider.probe.outcome} · {doctor.provider.probe.latency_ms} ms
                      </div>
                      <div className="mt-2 text-xs text-rc-text-secondary">{doctor.provider.probe.detail}</div>
                    </div>
                  )}

                  {doctor.network.length > 0 && (
                    <div className="rounded-md bg-rc-bg-tertiary px-3 py-3">
                      <div className="font-medium text-rc-text-primary">Network probes</div>
                      <div className="mt-2 space-y-2 text-xs text-rc-text-secondary">
                        {doctor.network.map((probe) => (
                          <div key={`${probe.label}-${probe.url}`}>
                            {probe.label}: {probe.outcome} · {probe.latency_ms} ms
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {doctor.env_providers.length > 0 && (
                    <div className="rounded-md bg-rc-bg-tertiary px-3 py-3">
                      <div className="font-medium text-rc-text-primary">Env providers</div>
                      <div className="mt-2 space-y-2 text-xs text-rc-text-secondary">
                        {doctor.env_providers.map((provider) => (
                          <div key={provider.name}>
                            {provider.name} · {provider.protocol} · {provider.model ?? '(default model)'}
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </details>
          </div>
        )}
      </section>

      <section className="space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-base font-semibold text-rc-text-primary">Diagnostics</h3>
            <p className="mt-1 text-sm text-rc-text-tertiary">
              导出本机诊断目录，包含日志和可选的脱敏配置快照。
            </p>
          </div>
          <button
            onClick={() => {
              void exportDiagnostics();
            }}
            disabled={diagnosticExporting || (!includeDiagnosticLogs && !includeDiagnosticSettings)}
            className="inline-flex items-center gap-2 rounded-md bg-rc-accent-primary px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-rc-accent-primary-hover disabled:cursor-not-allowed disabled:bg-rc-text-tertiary"
          >
            <Archive size={15} />
            {diagnosticExporting ? '导出中…' : '导出诊断包'}
          </button>
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <label className="flex items-center gap-3 rounded-md bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary">
            <input
              type="checkbox"
              checked={includeDiagnosticLogs}
              onChange={(event) => setIncludeDiagnosticLogs(event.target.checked)}
            />
            <span>包含日志</span>
          </label>
          <label className="flex items-center gap-3 rounded-md bg-rc-bg-surface px-4 py-3 text-sm text-rc-text-primary">
            <input
              type="checkbox"
              checked={includeDiagnosticSettings}
              onChange={(event) => setIncludeDiagnosticSettings(event.target.checked)}
            />
            <span>包含脱敏配置</span>
          </label>
        </div>

        {diagnosticError && (
          <div className="rounded-lg border border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-3 text-sm text-rc-accent-error">
            {diagnosticError}
          </div>
        )}

        {diagnosticPath && (
          <div className="rounded-lg border border-rc-border-primary bg-rc-bg-surface px-4 py-4">
            <div className="text-sm font-semibold text-rc-text-primary">诊断包已生成</div>
            <div className="mt-2 break-all rounded-md bg-rc-bg-tertiary px-3 py-3 font-mono text-xs text-rc-text-secondary">
              {formatSensitivePath(diagnosticPath, privacyMode)}
            </div>
          </div>
        )}
      </section>

      <section className="space-y-4">
        <div>
          <h3 className="text-base font-semibold text-rc-text-primary">Session Export</h3>
          <p className="mt-1 text-sm text-rc-text-tertiary">
            直接把当前或历史会话导出为 JSON bundle / NDJSON transcript，落到 runtime 默认导出目录。
          </p>
        </div>

        <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_220px]">
          <div className="space-y-1.5">
            <label className="block text-sm font-medium text-rc-text-primary">选择会话</label>
            <select
              value={selectedSessionId}
              onChange={(event) => setSelectedSessionId(event.target.value)}
              className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
            >
              {allSessions.length === 0 ? (
                <option value="">没有可导出的会话</option>
              ) : (
                allSessions.map((session) => (
                  <option key={session.id} value={session.id}>
                    {describeSession(session)}
                  </option>
                ))
              )}
            </select>
          </div>

          <div className="space-y-1.5">
            <label className="block text-sm font-medium text-rc-text-primary">导出格式</label>
            <select
              value={exportFormat}
              onChange={(event) => setExportFormat(event.target.value as SessionExportFormat)}
              className="w-full rounded-md border border-rc-border-primary bg-rc-bg-surface px-3 py-2.5 text-sm text-rc-text-primary outline-none transition-colors focus:border-rc-border-focus"
            >
              {EXPORT_FORMATS.map((format) => (
                <option key={format.value} value={format.value}>
                  {format.label}
                </option>
              ))}
            </select>
          </div>
        </div>

        <button
          onClick={() => {
            void exportSession();
          }}
          disabled={!selectedSessionId || exporting}
          className="inline-flex items-center gap-2 rounded-md bg-rc-accent-primary px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-rc-accent-primary-hover disabled:cursor-not-allowed disabled:bg-rc-text-tertiary"
        >
          <Download size={15} />
          {exporting ? '导出中…' : '导出会话'}
        </button>

        {exportError && (
          <div className="rounded-lg border border-rc-accent-error-border bg-rc-accent-error-bg px-4 py-3 text-sm text-rc-accent-error">
            {exportError}
          </div>
        )}

        {exportPath && (
          <div className="rounded-lg border border-rc-border-primary bg-rc-bg-surface px-4 py-4">
            <div className="text-sm font-semibold text-rc-text-primary">导出完成</div>
            <div className="mt-2 break-all rounded-md bg-rc-bg-tertiary px-3 py-3 font-mono text-xs text-rc-text-secondary">
              {exportPath}
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
