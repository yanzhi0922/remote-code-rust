import { Download, RefreshCw, ShieldCheck, Stethoscope } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { DoctorReportInfo, SessionExportFormat, SessionSummary } from '../../lib/types';
import * as tauri from '../../lib/tauri';
import { useAppStore } from '../../stores/useAppStore';

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
      className={`rounded-[24px] border px-4 py-4 ${
        tone === 'issue'
          ? 'border-rose-200 bg-rose-50 text-rose-800'
          : 'border-amber-200 bg-amber-50 text-amber-800'
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
      .runDoctorReport(false, false, true)
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
      const report = await tauri.runDoctorReport(probeNetwork, probeProvider, includeEnvProviders);
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

  return (
    <div className="space-y-6">
      <section className="space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-base font-semibold text-slate-800">Doctor</h3>
            <p className="mt-1 text-sm text-slate-500">
              在 GUI 内直接查看 runtime、provider、权限、扩展和网络可达性，不用切回 CLI。
            </p>
          </div>
          <button
            onClick={() => {
              void runDoctor();
            }}
            disabled={doctorLoading}
            className="inline-flex items-center gap-2 rounded-2xl border border-[#ddd6c8] bg-white px-4 py-2 text-sm font-medium text-slate-700 transition-colors hover:bg-[#faf8f3] disabled:cursor-not-allowed disabled:opacity-60"
          >
            <RefreshCw size={14} className={doctorLoading ? 'animate-spin' : ''} />
            {doctorLoading ? '诊断中…' : '重新诊断'}
          </button>
        </div>

        <div className="grid gap-3 md:grid-cols-3">
          <label className="flex items-center gap-3 rounded-2xl bg-white px-4 py-3 text-sm text-slate-700">
            <input
              type="checkbox"
              checked={probeProvider}
              onChange={(event) => setProbeProvider(event.target.checked)}
            />
            <span>探测 Provider</span>
          </label>
          <label className="flex items-center gap-3 rounded-2xl bg-white px-4 py-3 text-sm text-slate-700">
            <input
              type="checkbox"
              checked={probeNetwork}
              onChange={(event) => setProbeNetwork(event.target.checked)}
            />
            <span>探测网络</span>
          </label>
          <label className="flex items-center gap-3 rounded-2xl bg-white px-4 py-3 text-sm text-slate-700">
            <input
              type="checkbox"
              checked={includeEnvProviders}
              onChange={(event) => setIncludeEnvProviders(event.target.checked)}
            />
            <span>显示环境变量 Provider</span>
          </label>
        </div>

        {doctorError && (
          <div className="rounded-[24px] border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
            {doctorError}
          </div>
        )}

        {doctor && (
          <div className="space-y-4">
            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
              <div className="rounded-[24px] border border-[#ddd6c8] bg-white px-4 py-4">
                <div className="flex items-center gap-2 text-sm font-semibold text-slate-700">
                  <Stethoscope size={15} />
                  Readiness
                </div>
                <div
                  className={`mt-3 inline-flex rounded-full px-3 py-1 text-xs font-semibold ${
                    doctor.ok ? 'bg-emerald-100 text-emerald-700' : 'bg-rose-100 text-rose-700'
                  }`}
                >
                  {doctor.ok ? 'READY' : 'NOT READY'}
                </div>
                <div className="mt-3 text-xs text-slate-500">
                  {doctor.provider.name} · {doctor.provider.protocol}
                </div>
              </div>

              <div className="rounded-[24px] border border-[#ddd6c8] bg-white px-4 py-4">
                <div className="text-sm font-semibold text-slate-700">Runtime</div>
                <div className="mt-3 text-sm text-slate-700">{doctor.runtime.permission_mode}</div>
                <div className="mt-2 text-xs text-slate-500">
                  session {doctor.runtime.session_name ?? '(auto)'}
                </div>
                <div className="mt-1 text-xs text-slate-500">{doctor.runtime.version}</div>
              </div>

              <div className="rounded-[24px] border border-[#ddd6c8] bg-white px-4 py-4">
                <div className="text-sm font-semibold text-slate-700">Provider</div>
                <div className="mt-3 text-sm text-slate-700">
                  {doctor.provider.model ?? '(missing model)'}
                </div>
                <div className="mt-2 text-xs text-slate-500">
                  auth {doctor.provider.auth_source ?? '(missing)'}
                </div>
                <div className="mt-1 text-xs text-slate-500">
                  ctx {doctor.provider.context_window_tokens} / out {doctor.provider.output_reserve_tokens}
                </div>
              </div>

              <div className="rounded-[24px] border border-[#ddd6c8] bg-white px-4 py-4">
                <div className="flex items-center gap-2 text-sm font-semibold text-slate-700">
                  <ShieldCheck size={15} />
                  Surfaces
                </div>
                <div className="mt-3 text-sm text-slate-700">
                  tools {doctor.tools.builtin_tools} · rules {doctor.permissions.layered_rules}
                </div>
                <div className="mt-2 text-xs text-slate-500">
                  skills {doctor.extensions.skills} · plugins {doctor.extensions.plugins}
                </div>
                <div className="mt-1 text-xs text-slate-500">
                  mcp {doctor.extensions.managed_mcp_servers + doctor.extensions.plugin_mcp_servers}
                </div>
              </div>
            </div>

            <DoctorList title="Issues" items={doctor.issues} tone="issue" />
            <DoctorList title="Warnings" items={doctor.warnings} tone="warning" />

            <details className="rounded-[24px] border border-[#ddd6c8] bg-white px-4 py-4">
              <summary className="cursor-pointer text-sm font-semibold text-slate-700">
                详细信息
              </summary>
              <div className="mt-4 grid gap-4 lg:grid-cols-2">
                <div className="space-y-2 text-sm text-slate-600">
                  <div>
                    <span className="font-medium text-slate-700">CWD:</span> {doctor.runtime.cwd}
                  </div>
                  <div>
                    <span className="font-medium text-slate-700">Profile:</span> {doctor.runtime.profile_dir}
                  </div>
                  <div>
                    <span className="font-medium text-slate-700">Setting sources:</span>{' '}
                    {doctor.runtime.setting_sources.length > 0
                      ? doctor.runtime.setting_sources.join(', ')
                      : '(defaults)'}
                  </div>
                  <div>
                    <span className="font-medium text-slate-700">Allowed tools:</span>{' '}
                    {doctor.tools.allowed_tools.length > 0 ? doctor.tools.allowed_tools.join(', ') : '(all)'}
                  </div>
                  <div>
                    <span className="font-medium text-slate-700">Denied tools:</span>{' '}
                    {doctor.tools.disallowed_tools.length > 0
                      ? doctor.tools.disallowed_tools.join(', ')
                      : '(none)'}
                  </div>
                </div>

                <div className="space-y-3 text-sm text-slate-600">
                  {doctor.provider.probe && (
                    <div className="rounded-2xl bg-[#f7f4ed] px-3 py-3">
                      <div className="font-medium text-slate-700">Provider probe</div>
                      <div className="mt-1 text-xs text-slate-500">
                        {doctor.provider.probe.outcome} · {doctor.provider.probe.latency_ms} ms
                      </div>
                      <div className="mt-2 text-xs text-slate-600">{doctor.provider.probe.detail}</div>
                    </div>
                  )}

                  {doctor.network.length > 0 && (
                    <div className="rounded-2xl bg-[#f7f4ed] px-3 py-3">
                      <div className="font-medium text-slate-700">Network probes</div>
                      <div className="mt-2 space-y-2 text-xs text-slate-600">
                        {doctor.network.map((probe) => (
                          <div key={`${probe.label}-${probe.url}`}>
                            {probe.label}: {probe.outcome} · {probe.latency_ms} ms
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {doctor.env_providers.length > 0 && (
                    <div className="rounded-2xl bg-[#f7f4ed] px-3 py-3">
                      <div className="font-medium text-slate-700">Env providers</div>
                      <div className="mt-2 space-y-2 text-xs text-slate-600">
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
        <div>
          <h3 className="text-base font-semibold text-slate-800">Session Export</h3>
          <p className="mt-1 text-sm text-slate-500">
            直接把当前或历史会话导出为 JSON bundle / NDJSON transcript，落到 runtime 默认导出目录。
          </p>
        </div>

        <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_220px]">
          <div className="space-y-1.5">
            <label className="block text-sm font-medium text-slate-700">选择会话</label>
            <select
              value={selectedSessionId}
              onChange={(event) => setSelectedSessionId(event.target.value)}
              className="w-full rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
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
            <label className="block text-sm font-medium text-slate-700">导出格式</label>
            <select
              value={exportFormat}
              onChange={(event) => setExportFormat(event.target.value as SessionExportFormat)}
              className="w-full rounded-2xl border border-[#ddd6c8] bg-white px-3 py-2.5 text-sm outline-none transition-colors focus:border-slate-500"
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
          className="inline-flex items-center gap-2 rounded-2xl bg-[#17181a] px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-[#2b2d31] disabled:cursor-not-allowed disabled:bg-[#c9c2b5]"
        >
          <Download size={15} />
          {exporting ? '导出中…' : '导出会话'}
        </button>

        {exportError && (
          <div className="rounded-[24px] border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
            {exportError}
          </div>
        )}

        {exportPath && (
          <div className="rounded-[24px] border border-[#ddd6c8] bg-white px-4 py-4">
            <div className="text-sm font-semibold text-slate-700">导出完成</div>
            <div className="mt-2 break-all rounded-2xl bg-[#f7f4ed] px-3 py-3 font-mono text-xs text-slate-600">
              {exportPath}
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
