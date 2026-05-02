import { useState } from 'react';
import type { FullSettings } from '../../lib/types';
import { SettingInput } from './SettingInput';

export interface ProviderSettingsProps {
  settings: FullSettings;
  onUpdate: (updates: Record<string, unknown>) => void;
}

const PROTOCOLS = [
  { value: 'openai', label: 'OpenAI / 兼容 Chat Completions' },
  { value: 'anthropic', label: 'Anthropic Messages' },
  { value: 'bedrock', label: 'AWS Bedrock' },
  { value: 'vertex', label: 'Google Vertex' },
];

const PREDEFINED_MODELS: Record<string, string[]> = {
  openai: ['gpt-4', 'gpt-4o', 'gpt-4o-mini', 'gpt-3.5-turbo', 'o1', 'o1-mini', 'o3-mini'],
  anthropic: ['claude-sonnet-4-20250514', 'claude-opus-4-7', 'claude-sonnet-4-6', 'claude-haiku-4-5', 'claude-3-5-sonnet-20241022', 'claude-3-5-haiku-20241022', 'claude-3-opus-20240229'],
  bedrock: ['anthropic.claude-sonnet-4-20250514-v1:0', 'anthropic.claude-3-5-sonnet', 'anthropic.claude-3-opus', 'anthropic.claude-3-haiku'],
  vertex: ['claude-sonnet-4@20250514', 'claude-3-5-sonnet', 'claude-3-opus', 'claude-3-haiku'],
};

function isValidUrl(url: string): boolean {
  if (!url) return true;
  try {
    new URL(url);
    return true;
  } catch {
    return false;
  }
}

export function ProviderSettings({ settings, onUpdate }: ProviderSettingsProps) {
  const [customModel, setCustomModel] = useState(false);
  const [urlError, setUrlError] = useState<string | null>(null);

  const models = PREDEFINED_MODELS[settings.provider_protocol] ?? [];

  function handleBaseUrlChange(value: string) {
    if (value && !isValidUrl(value)) {
      setUrlError('请输入有效的 URL');
    } else {
      setUrlError(null);
    }
    onUpdate({ provider_base_url: value || null });
  }

  return (
    <div className="space-y-6" data-testid="provider-settings">
      <h3 className="text-lg font-semibold text-slate-800">提供商设置</h3>

      <div className="space-y-1.5">
        <label className="block text-sm font-medium text-slate-700">提供商名称</label>
        <input
          type="text"
          value={settings.provider_name}
          onChange={(e) => onUpdate({ provider_name: e.target.value })}
          className="w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-800 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          aria-label="提供商名称"
          data-testid="provider-name-input"
        />
      </div>

      <div className="space-y-1.5">
        <label className="block text-sm font-medium text-slate-700">协议</label>
        <select
          value={settings.provider_protocol}
          onChange={(e) => {
            onUpdate({ provider_protocol: e.target.value });
            setCustomModel(false);
          }}
          className="w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-800 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          aria-label="协议"
          data-testid="protocol-select"
        >
          {PROTOCOLS.map((p) => (
            <option key={p.value} value={p.value}>
              {p.label}
            </option>
          ))}
        </select>
      </div>

      <div className="space-y-1.5">
        <div className="flex items-center justify-between">
          <label className="block text-sm font-medium text-slate-700">模型</label>
          <button
            type="button"
            onClick={() => setCustomModel((prev) => !prev)}
            className="text-xs text-blue-600 hover:text-blue-800"
            data-testid="toggle-custom-model"
          >
            {customModel ? '选择预定义' : '自定义输入'}
          </button>
        </div>
        {customModel ? (
          <input
            type="text"
            value={settings.provider_model ?? ''}
            onChange={(e) => onUpdate({ provider_model: e.target.value || null })}
            placeholder="输入自定义模型名称"
            className="w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-800 placeholder:text-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
            data-testid="custom-model-input"
          />
        ) : (
          <select
            value={settings.provider_model ?? ''}
            onChange={(e) => onUpdate({ provider_model: e.target.value || null })}
            className="w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-800 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
            aria-label="模型"
            data-testid="model-select"
          >
            <option value="">-- 选择模型 --</option>
            {models.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        )}
      </div>

      <div className="space-y-1.5">
        <SettingInput
          label="Base URL"
          description="API 基础地址，留空使用默认值"
          value={settings.provider_base_url ?? ''}
          onChange={handleBaseUrlChange}
          placeholder="https://api.openai.com"
        />
        {urlError && <p className="text-xs text-red-500" data-testid="url-error">{urlError}</p>}
      </div>

      <SettingInput
        label="API Key"
        description={settings.provider_api_key_set ? 'API Key 已设置' : '尚未设置 API Key'}
        value=""
        type="password"
        onChange={(value) => onUpdate({ api_key: value })}
        placeholder={settings.provider_api_key_set ? '输入新值以替换' : '输入 API Key'}
      />
    </div>
  );
}
