import { useState } from 'react';
import type { ConfigScope, McpServerDraft } from '../../lib/types';

interface McpAddServerFormProps {
  onSubmit: (draft: McpServerDraft) => void;
  onCancel: () => void;
  scope: ConfigScope;
}

interface FormErrors {
  name?: string;
  command?: string;
  url?: string;
}

export function McpAddServerForm({ onSubmit, onCancel, scope }: McpAddServerFormProps) {
  const [name, setName] = useState('');
  const [transport, setTransport] = useState<'stdio' | 'http' | 'websocket'>('stdio');
  const [command, setCommand] = useState('');
  const [url, setUrl] = useState('');
  const [argsText, setArgsText] = useState('');
  const [cwd, setCwd] = useState('');
  const [envText, setEnvText] = useState('');
  const [headersText, setHeadersText] = useState('');
  const [startupTimeout, setStartupTimeout] = useState('');
  const [requestTimeout, setRequestTimeout] = useState('');
  const [disabled, setDisabled] = useState(false);
  const [errors, setErrors] = useState<FormErrors>({});

  function validate(): FormErrors {
    const errs: FormErrors = {};
    if (!name.trim()) {
      errs.name = '名称不能为空';
    }
    if (transport === 'stdio' && !command.trim()) {
      errs.command = 'stdio 类型必须填写命令';
    }
    if ((transport === 'http' || transport === 'websocket') && !url.trim()) {
      errs.url = `${transport} 类型必须填写 URL`;
    }
    return errs;
  }

  function parseKeyValueLines(value: string): Record<string, string> {
    const result: Record<string, string> = {};
    for (const line of value.split(/\r?\n/).map((l) => l.trim()).filter(Boolean)) {
      const idx = line.indexOf('=');
      if (idx > 0) {
        result[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
      }
    }
    return result;
  }

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const validationErrors = validate();
    if (Object.keys(validationErrors).length > 0) {
      setErrors(validationErrors);
      return;
    }
    setErrors({});

    const draft: McpServerDraft = {
      scope,
      name: name.trim(),
      transport,
      command: transport === 'stdio' ? command.trim() : null,
      url: transport !== 'stdio' ? url.trim() : null,
      args: transport === 'stdio' && argsText.trim() ? argsText.split(/\s+/) : undefined,
      cwd: cwd.trim() || null,
      env: envText.trim() ? parseKeyValueLines(envText) : undefined,
      headers: headersText.trim() ? parseKeyValueLines(headersText) : undefined,
      disabled: disabled || undefined,
      startup_timeout_secs: startupTimeout.trim() ? Number(startupTimeout) : null,
      request_timeout_secs: requestTimeout.trim() ? Number(requestTimeout) : null,
    };

    onSubmit(draft);
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4" data-testid="mcp-add-server-form">
      <h3 className="text-lg font-semibold text-slate-800">添加 MCP 服务器</h3>

      {/* Name */}
      <div>
        <label className="mb-1 block text-sm font-medium text-slate-700">
          名称 <span className="text-red-500">*</span>
        </label>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-300 focus:outline-none"
          placeholder="my-server"
          data-testid="mcp-form-name"
        />
        {errors.name && <span className="mt-1 text-xs text-red-500">{errors.name}</span>}
      </div>

      {/* Transport */}
      <div>
        <label className="mb-1 block text-sm font-medium text-slate-700">传输类型</label>
        <div className="flex gap-2">
          {(['stdio', 'http', 'websocket'] as const).map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => setTransport(t)}
              className={`rounded-xl px-3 py-1.5 text-sm font-medium ${
                transport === t
                  ? 'bg-emerald-600 text-white'
                  : 'border border-slate-200 text-slate-600 hover:bg-slate-50'
              }`}
              data-testid={`mcp-form-transport-${t}`}
            >
              {t.toUpperCase()}
            </button>
          ))}
        </div>
      </div>

      {/* stdio fields */}
      {transport === 'stdio' && (
        <>
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-700">
              命令 <span className="text-red-500">*</span>
            </label>
            <input
              type="text"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-300 focus:outline-none"
              placeholder="node /path/to/server.js"
              data-testid="mcp-form-command"
            />
            {errors.command && <span className="mt-1 text-xs text-red-500">{errors.command}</span>}
          </div>
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-700">参数（空格分隔）</label>
            <input
              type="text"
              value={argsText}
              onChange={(e) => setArgsText(e.target.value)}
              className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-300 focus:outline-none"
              placeholder="--port 3000"
              data-testid="mcp-form-args"
            />
          </div>
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-700">工作目录</label>
            <input
              type="text"
              value={cwd}
              onChange={(e) => setCwd(e.target.value)}
              className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-300 focus:outline-none"
              placeholder="/path/to/project"
              data-testid="mcp-form-cwd"
            />
          </div>
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-700">环境变量（每行 KEY=VALUE）</label>
            <textarea
              value={envText}
              onChange={(e) => setEnvText(e.target.value)}
              className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-300 focus:outline-none"
              placeholder={"API_KEY=xxx\nDEBUG=true"}
              rows={3}
              data-testid="mcp-form-env"
            />
          </div>
        </>
      )}

      {/* http/websocket fields */}
      {(transport === 'http' || transport === 'websocket') && (
        <>
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-700">
              URL <span className="text-red-500">*</span>
            </label>
            <input
              type="text"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-300 focus:outline-none"
              placeholder={transport === 'http' ? 'http://localhost:8080/mcp' : 'ws://localhost:8080/mcp'}
              data-testid="mcp-form-url"
            />
            {errors.url && <span className="mt-1 text-xs text-red-500">{errors.url}</span>}
          </div>
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-700">请求头（每行 KEY=VALUE）</label>
            <textarea
              value={headersText}
              onChange={(e) => setHeadersText(e.target.value)}
              className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-300 focus:outline-none"
              placeholder={"Authorization=Bearer xxx"}
              rows={3}
              data-testid="mcp-form-headers"
            />
          </div>
        </>
      )}

      {/* Common fields */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="mb-1 block text-sm font-medium text-slate-700">启动超时（秒）</label>
          <input
            type="number"
            value={startupTimeout}
            onChange={(e) => setStartupTimeout(e.target.value)}
            className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-300 focus:outline-none"
            placeholder="30"
            data-testid="mcp-form-startup-timeout"
          />
        </div>
        <div>
          <label className="mb-1 block text-sm font-medium text-slate-700">请求超时（秒）</label>
          <input
            type="number"
            value={requestTimeout}
            onChange={(e) => setRequestTimeout(e.target.value)}
            className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm focus:border-emerald-300 focus:outline-none"
            placeholder="60"
            data-testid="mcp-form-request-timeout"
          />
        </div>
      </div>

      <label className="flex items-center gap-2 text-sm text-slate-700">
        <input
          type="checkbox"
          checked={disabled}
          onChange={(e) => setDisabled(e.target.checked)}
          className="rounded border-slate-300"
          data-testid="mcp-form-disabled"
        />
        禁用此服务器
      </label>

      {/* Buttons */}
      <div className="flex items-center gap-2">
        <button
          type="submit"
          className="rounded-xl bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-700"
          data-testid="mcp-form-submit"
        >
          添加
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-xl border border-slate-200 px-4 py-2 text-sm text-slate-600 hover:bg-slate-50"
          data-testid="mcp-form-cancel"
        >
          取消
        </button>
      </div>
    </form>
  );
}
